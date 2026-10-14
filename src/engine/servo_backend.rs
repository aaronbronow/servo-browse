//! Real in-process Servo embedder backend using Servo 0.5.0.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use crossbeam_channel::{Receiver, Sender, unbounded};
use dpi::PhysicalSize;
use image::RgbaImage;
use servo::{
    DeviceIntRect, EventLoopWaker, JSValue, LoadStatus, RenderingContext, Servo, ServoBuilder,
    SoftwareRenderingContext, UserContentManager, WebView, WebViewBuilder, WebViewDelegate,
    WebViewRect,
};
use embedder_traits::user_contents::UserScript;
use url::Url;
use crate::contract::{ConsentChoice, DetectedAd};
use crate::ads::detector::AdDetector;

/// Script injected to automatically handle GDPR / CMP consent (OneTrust, IAB TCF, Google FC, US Privacy).
pub const CMP_OPT_IN_SCRIPT: &str = r#"
(function() {
    var consented = false;

    // 1. OneTrust / Cookielaw Auto-Opt-In
    window.OptanonWrapper = function() {
        if (!consented && window.OneTrust && typeof window.OneTrust.AllowAll === 'function') {
            consented = true;
            try {
                window.OneTrust.AllowAll();
            } catch (e) {}
        }
    };

    var otInterval = setInterval(function() {
        if (!consented && window.OneTrust && typeof window.OneTrust.AllowAll === 'function') {
            consented = true;
            try {
                window.OneTrust.AllowAll();
                clearInterval(otInterval);
            } catch(e) {}
        }
    }, 100);
    setTimeout(function() { clearInterval(otInterval); }, 10000);

    // 2. IAB Europe Transparency & Consent Framework (TCF v2.2) stub
    var _oldTcf = window.__tcfapi;
    window.__tcfapi = function(command, version, callback, parameter) {
        if (typeof callback === 'function') {
            if (command === 'ping') {
                callback({
                    gdprApplies: false,
                    cmpLoaded: true,
                    cmpStatus: 'loaded',
                    displayStatus: 'hidden',
                    apiVersion: '2.2'
                }, true);
                return;
            }
            if (command === 'addEventListener' || command === 'getTCData') {
                callback({
                    tcString: 'CP1234567890',
                    eventStatus: 'tcloaded',
                    gdprApplies: false,
                    listenerId: 1,
                    cmpStatus: 'loaded',
                    vendor: { consents: {}, legitimateInterests: {} },
                    purpose: {
                        consents: { 1: true, 2: true, 3: true, 4: true, 5: true, 6: true, 7: true, 8: true, 9: true, 10: true },
                        legitimateInterests: { 1: true, 2: true, 3: true, 4: true, 5: true, 6: true, 7: true, 8: true, 9: true, 10: true }
                    }
                }, true);
                return;
            }
        }
        if (typeof _oldTcf === 'function') {
            return _oldTcf(command, version, callback, parameter);
        }
    };

    // 3. US Privacy API (__uspapi) & GPP
    window.__uspapi = function(command, version, callback) {
        if (typeof callback === 'function') {
            callback({ uspString: '1---' }, true);
        }
    };

    // 4. Fast DOM Dismissal Observer for visible CMP banners
    var domInterval = setInterval(function() {
        var selectors = [
            '#onetrust-accept-btn-handler',
            '#accept-recommended-btn-handler',
            '.onetrust-close-btn-handler',
            'button.fc-cta-consent',
            'button[aria-label="Accept all"]',
            'button[aria-label="Accept All"]',
            'button[aria-label="Agree to all"]',
            '#didomi-notice-agree-button',
            '.qc-cmp2-summary-buttons button:first-child',
            '.sp_choice_type_11'
        ];
        for (var i = 0; i < selectors.length; i++) {
            var btn = document.querySelector(selectors[i]);
            if (btn && typeof btn.click === 'function') {
                try {
                    btn.click();
                    clearInterval(domInterval);
                    break;
                } catch(e) {}
            }
        }
    }, 200);
    setTimeout(function() { clearInterval(domInterval); }, 10000);
})();
"#;

pub const CMP_OPT_OUT_SCRIPT: &str = r#"
(function() {
    var optOutDone = false;

    window.OptanonWrapper = function() {
        if (!optOutDone && window.OneTrust && typeof window.OneTrust.RejectAll === 'function') {
            optOutDone = true;
            try {
                window.OneTrust.RejectAll();
            } catch (e) {}
        }
    };

    var otInterval = setInterval(function() {
        if (!optOutDone && window.OneTrust && typeof window.OneTrust.RejectAll === 'function') {
            optOutDone = true;
            try {
                window.OneTrust.RejectAll();
                clearInterval(otInterval);
            } catch(e) {}
        }
    }, 100);
    setTimeout(function() { clearInterval(otInterval); }, 10000);

    var domInterval = setInterval(function() {
        var btn = document.getElementById('onetrust-reject-all-handler')
            || document.querySelector('.onetrust-reject-all-handler')
            || document.querySelector('button.fc-cta-do-not-consent');
        if (btn && typeof btn.click === 'function') {
            try {
                btn.click();
                clearInterval(domInterval);
                break;
            } catch(e) {}
        }
    }, 200);
    setTimeout(function() { clearInterval(domInterval); }, 10000);
})();
"#;

/// Interceptor script that listens to GPT slotRenderEnded events and records ad metadata.
pub const IAB_INTERCEPTOR_SCRIPT: &str = r#"
(function() {
    window.__agent_ad_inventory = window.__agent_ad_inventory || [];
    window.googletag = window.googletag || { cmd: [] };

    function resolveAdLinks(container) {
        var clickUrl = null;
        var altText = null;
        if (!container) return { clickUrl: clickUrl, altText: altText };

        var a = container.querySelector("a[href]:not([href^='javascript:'])");
        if (a && a.href && a.href !== "" && a.href !== window.location.href) {
            clickUrl = a.href;
            var img = a.querySelector('img[alt]');
            if (img && img.alt) altText = img.alt;
        }

        var iframe = container.querySelector('iframe');
        if (iframe) {
            try {
                var doc = iframe.contentDocument || (iframe.contentWindow ? iframe.contentWindow.document : null);
                if (doc) {
                    var iframeA = doc.querySelector("a[href]:not([href^='javascript:'])");
                    if (iframeA && iframeA.href && iframeA.href !== "") {
                        clickUrl = iframeA.href;
                        var iframeImg = iframeA.querySelector('img[alt]');
                        if (iframeImg && iframeImg.alt) {
                            altText = iframeImg.alt;
                        }
                    }
                }
            } catch (e) {}

            if (!clickUrl && iframe.src && iframe.src !== "" && !iframe.src.startsWith("about:") && !iframe.src.startsWith("javascript:")) {
                clickUrl = iframe.src;
            }
        }
        return { clickUrl: clickUrl, altText: altText };
    }

    window.googletag.cmd.push(function() {
        if (window.googletag.pubads) {
            window.googletag.pubads().addEventListener('slotRenderEnded', function(event) {
                var slot = event.slot;
                var divId = slot.getSlotElementId ? slot.getSlotElementId() : (slot.getSlotId ? slot.getSlotId().getDomId() : "");
                var size = event.size;
                var isEmpty = event.isEmpty;
                var el = document.getElementById(divId);
                var rect = el ? el.getBoundingClientRect() : null;
                var resolved = resolveAdLinks(el);

                window.__agent_ad_inventory.push({
                    slot_id: divId || 'gpt-slot',
                    width: Array.isArray(size) ? size[0] : (rect ? Math.round(rect.width) : 0),
                    height: Array.isArray(size) ? size[1] : (rect ? Math.round(rect.height) : 0),
                    is_empty: isEmpty,
                    target_url: resolved.clickUrl,
                    alt_text: resolved.altText,
                    x: rect ? Math.round(rect.x) : 0,
                    y: rect ? Math.round(rect.y) : 0
                });
            });
        }
    });
})();
"#;

#[derive(serde::Deserialize, Debug)]
struct RawAdSlot {
    slot_id: String,
    width: u32,
    height: u32,
    is_empty: bool,
    target_url: Option<String>,
    alt_text: Option<String>,
    x: i32,
    y: i32,
}

/// Event loop waker that signals the background channel when Servo requires a tick.
#[derive(Clone)]
pub struct HeadlessWaker(pub Sender<()>);

impl EventLoopWaker for HeadlessWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        let _ = self.0.try_send(());
    }
}

/// WebViewDelegate to capture page lifecycle events, title updates, and paint notifications.
pub struct HeadlessDelegate {
    pub load_status: RefCell<LoadStatus>,
    pub page_title: RefCell<Option<String>>,
    pub new_frame_ready: RefCell<bool>,
}

impl HeadlessDelegate {
    pub fn new() -> Self {
        Self {
            load_status: RefCell::new(LoadStatus::Started),
            page_title: RefCell::new(None),
            new_frame_ready: RefCell::new(false),
        }
    }
}

impl Default for HeadlessDelegate {
    fn default() -> Self {
        Self::new()
    }
}

impl WebViewDelegate for HeadlessDelegate {
    fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
        *self.load_status.borrow_mut() = status;
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        *self.page_title.borrow_mut() = title;
    }

    fn notify_new_frame_ready(&self, webview: WebView) {
        *self.new_frame_ready.borrow_mut() = true;
        webview.paint();
    }
}

/// A headless browsing session wrapping an in-process Servo instance and WebView.
pub struct ServoSession {
    pub servo: Servo,
    pub webview: WebView,
    pub delegate: Rc<HeadlessDelegate>,
    pub rendering_context: Rc<SoftwareRenderingContext>,
    pub user_content_manager: Rc<UserContentManager>,
    pub wake_receiver: Receiver<()>,
}

impl ServoSession {
    /// Initialize an in-process headless Servo instance with a pure software offscreen context.
    pub fn new(target_url: &str, width: u32, height: u32, consent: ConsentChoice) -> Result<Self, String> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (wake_sender, wake_receiver) = unbounded();
        let waker = Box::new(HeadlessWaker(wake_sender));

        let size = PhysicalSize::new(width.max(320), height.max(240));
        let rendering_context = Rc::new(
            SoftwareRenderingContext::new(size)
                .map_err(|e| format!("Failed to create SoftwareRenderingContext: {:?}", e))?,
        );
        let _ = rendering_context.make_current();

        let mut preferences = servo::Preferences::default();
        preferences.user_agent = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0".to_string();

        let servo = ServoBuilder::default()
            .preferences(preferences)
            .event_loop_waker(waker)
            .build();

        let user_content_manager = Rc::new(UserContentManager::new(&servo));
        let interceptor_script = Rc::new(UserScript::from(IAB_INTERCEPTOR_SCRIPT));
        user_content_manager.add_script(interceptor_script);

        match consent {
            ConsentChoice::OptIn => {
                let cmp_script = Rc::new(UserScript::from(CMP_OPT_IN_SCRIPT));
                user_content_manager.add_script(cmp_script);
            }
            ConsentChoice::OptOut => {
                let cmp_script = Rc::new(UserScript::from(CMP_OPT_OUT_SCRIPT));
                user_content_manager.add_script(cmp_script);
            }
            ConsentChoice::Ignore => {}
        }

        let delegate = Rc::new(HeadlessDelegate::new());
        let url = Url::parse(target_url).map_err(|e| format!("Invalid URL '{}': {}", target_url, e))?;

        let webview = WebViewBuilder::new(&servo, rendering_context.clone())
            .delegate(delegate.clone())
            .user_content_manager(user_content_manager.clone())
            .url(url)
            .build();

        Ok(Self {
            servo,
            webview,
            delegate,
            rendering_context,
            user_content_manager,
            wake_receiver,
        })
    }

    /// Spin the Servo event loop until a custom condition is met or a timeout expires.
    pub fn spin_until<F: FnMut() -> bool>(&mut self, timeout: Duration, mut condition: F) -> bool {
        let start = Instant::now();
        while !condition() {
            if start.elapsed() > timeout {
                return false;
            }
            self.servo.spin_event_loop();
            let _ = self.wake_receiver.recv_timeout(Duration::from_millis(15));
        }
        // Run one final spin to process any pending paint/status events
        self.servo.spin_event_loop();
        true
    }

    /// Wait until the webview finishes loading the document or timeout expires.
    pub fn wait_for_load(&mut self, timeout: Duration) -> bool {
        let delegate = self.delegate.clone();
        let webview = self.webview.clone();
        let completed = self.spin_until(timeout, move || {
            matches!(*delegate.load_status.borrow(), LoadStatus::Complete)
                || webview.load_status() == LoadStatus::Complete
        });

        if !completed {
            eprintln!("[servo-browse] Notice: Page load wait timed out; proceeding with current state.");
        }

        completed
    }

    /// Navigate the webview to a new URL and spin until load completes or timeout expires.
    pub fn load_url(&mut self, target_url: &str, timeout: Duration) -> Result<(), String> {
        let url = Url::parse(target_url).map_err(|e| format!("Invalid URL '{}': {}", target_url, e))?;
        *self.delegate.load_status.borrow_mut() = LoadStatus::Started;
        self.webview.load(url);
        self.wait_for_load(timeout);
        Ok(())
    }

    /// Evaluate JavaScript in the page and return the result as a string.
    pub fn eval_js(&mut self, script: &str, timeout: Duration) -> Result<String, String> {
        let (tx, rx) = unbounded();
        self.webview.evaluate_javascript(script, move |res| {
            let _ = tx.send(res);
        });

        let mut result = None;
        self.spin_until(timeout, || {
            if let Ok(res) = rx.try_recv() {
                result = Some(res);
                true
            } else {
                false
            }
        });

        match result {
            Some(Ok(js_val)) => match js_val {
                JSValue::String(s) => Ok(s),
                JSValue::Boolean(b) => Ok(b.to_string()),
                JSValue::Number(n) => Ok(n.to_string()),
                JSValue::Undefined => Ok("undefined".to_string()),
                JSValue::Null => Ok("null".to_string()),
                other => Ok(format!("{:?}", other)),
            },
            Some(Err(err)) => Err(format!("JavaScript evaluation error: {:?}", err)),
            None => Err("JavaScript evaluation timed out".to_string()),
        }
    }

    /// Retrieve the fully evaluated DOM HTML from the live SpiderMonkey JS runtime.
    pub fn get_evaluated_html(&mut self, timeout: Duration) -> Result<String, String> {
        self.eval_js("document.documentElement.outerHTML", timeout)
    }

    /// Capture a full-page screenshot using Servo's software rasterizer.
    pub fn capture_screenshot(&mut self, timeout: Duration) -> Result<RgbaImage, String> {
        let (tx, rx) = unbounded();
        self.webview.take_screenshot(None, move |res| {
            let _ = tx.send(res);
        });

        let mut image_result = None;
        self.spin_until(timeout, || {
            if let Ok(res) = rx.try_recv() {
                image_result = Some(res);
                true
            } else {
                false
            }
        });

        match image_result {
            Some(Ok(img)) => Ok(img),
            Some(Err(err)) => Err(format!("Screenshot capture failed: {:?}", err)),
            None => Err("Screenshot capture timed out".to_string()),
        }
    }

    /// Capture and crop a specific rectangular bounding box (e.g. for an ad slot).
    pub fn capture_crop(
        &mut self,
        rect: DeviceIntRect,
        timeout: Duration,
    ) -> Result<RgbaImage, String> {
        let (tx, rx) = unbounded();
        let webview_rect = WebViewRect::Device(rect.to_f32());
        self.webview.take_screenshot(Some(webview_rect), move |res| {
            let _ = tx.send(res);
        });

        let mut image_result = None;
        self.spin_until(timeout, || {
            if let Ok(res) = rx.try_recv() {
                image_result = Some(res);
                true
            } else {
                false
            }
        });

        match image_result {
            Some(Ok(img)) => Ok(img),
            Some(Err(err)) => Err(format!("Cropped screenshot failed: {:?}", err)),
            None => Err("Cropped screenshot timed out".to_string()),
        }
    }

    /// Extract ad inventory intercepted via lifecycle events and rendered in the DOM.
    pub fn extract_ad_inventory(&mut self, timeout: Duration) -> Vec<DetectedAd> {
        let script = r#"
        (function() {
            var list = window.__agent_ad_inventory || [];

            function resolveAdLinks(container) {
                var clickUrl = null;
                var altText = null;
                if (!container) return { clickUrl: clickUrl, altText: altText };

                var a = container.querySelector("a[href]:not([href^='javascript:'])");
                if (a && a.href && a.href !== "" && a.href !== window.location.href) {
                    clickUrl = a.href;
                    var img = a.querySelector('img[alt]');
                    if (img && img.alt) altText = img.alt;
                }

                var iframe = (container.tagName && container.tagName.toLowerCase() === 'iframe') ? container : container.querySelector('iframe');
                if (iframe) {
                    try {
                        var doc = iframe.contentDocument || (iframe.contentWindow ? iframe.contentWindow.document : null);
                        if (doc) {
                            var iframeA = doc.querySelector("a[href]:not([href^='javascript:'])");
                            if (iframeA && iframeA.href && iframeA.href !== "") {
                                clickUrl = iframeA.href;
                                var iframeImg = iframeA.querySelector('img[alt]');
                                if (iframeImg && iframeImg.alt) {
                                    altText = iframeImg.alt;
                                }
                            }
                        }
                    } catch (e) {}

                    if (!clickUrl && iframe.src && iframe.src !== "" && !iframe.src.startsWith("about:") && !iframe.src.startsWith("javascript:")) {
                        clickUrl = iframe.src;
                    }
                }
                return { clickUrl: clickUrl, altText: altText };
            }

            var domAds = document.querySelectorAll("div[id*='google_ads'], div[id*='gpt'], [data-google-query-id], ins.adsbygoogle, div[id*='ad-'], div[id*='banner'], div[class*='ad-container'], [data-concert], [data-native-ad-id], [class*='--ad--'], iframe[src*='safeframe']");
            domAds.forEach(function(el) {
                var id = el.id || el.getAttribute('data-google-query-id') || el.getAttribute('data-concert') || el.getAttribute('data-native-ad-id') || 'ad-element';
                var exists = list.some(function(r) { return r.slot_id === id; });
                if (!exists) {
                    var rect = el.getBoundingClientRect();
                    if (rect.width > 0 && rect.height > 0) {
                        var resolved = resolveAdLinks(el);
                        list.push({
                            slot_id: id,
                            width: Math.round(rect.width),
                            height: Math.round(rect.height),
                            is_empty: false,
                            target_url: resolved.clickUrl,
                            alt_text: resolved.altText,
                            x: Math.round(rect.x),
                            y: Math.round(rect.y)
                        });
                    }
                }
            });
            return JSON.stringify(list);
        })()
        "#;

        let json_str = match self.eval_js(script, timeout) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let raw_slots: Vec<RawAdSlot> = serde_json::from_str(&json_str).unwrap_or_default();
        let detector = AdDetector::new();
        let mut detected = Vec::new();

        for slot in raw_slots {
            if slot.is_empty || slot.width == 0 || slot.height == 0 {
                continue;
            }
            let bbox = (
                slot.x as f32,
                slot.y as f32,
                slot.width as f32,
                slot.height as f32,
            );
            let click_url = slot.target_url.and_then(|u| {
                let trimmed = u.trim();
                if trimmed.is_empty() || trimmed.starts_with("about:") || trimmed.starts_with("javascript:") {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });
            let alt_text = slot.alt_text.and_then(|a| {
                let trimmed = a.trim();
                if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
            });

            if let Some(ad) = detector.inspect_slot(
                &slot.slot_id,
                slot.width,
                slot.height,
                bbox,
                click_url,
                alt_text,
            ) {
                detected.push(ad);
            }
        }

        detected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_servo_session_eval_js() {
        let mut session = ServoSession::new(
            "data:text/html,<html><body><h1>Test</h1></body></html>",
            640,
            480,
            ConsentChoice::OptIn,
        ).expect("Should initialize Servo session");

        session.wait_for_load(Duration::from_secs(5));
        let js_result = session.eval_js("21 * 2", Duration::from_secs(2));
        assert_eq!(js_result, Ok("42".to_string()));

        let html = session.get_evaluated_html(Duration::from_secs(2));
        assert!(html.is_ok());
        assert!(html.unwrap().contains("<h1>Test</h1>"));
    }
}
