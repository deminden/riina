//! Own the AppKit controls for one player window.
mod preview;
mod slider;

use super::pip_window::player_window;
use crate::PlayerWindow;
use objc2::{
    DefinedClass, MainThreadOnly, define_class,
    rc::{Retained, Weak},
    runtime::{NSObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    self as kit, NSButton, NSCellImagePosition, NSCustomTouchBarItem, NSFont, NSImage, NSSlider,
    NSSliderTouchBarItem, NSTextField, NSTouchBar, NSTouchBarDelegate, NSTouchBarItem,
    NSTouchBarItemIdentifier, NSView, NSWindow,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSString};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

pub(crate) use preview::Preview;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Action {
    TogglePlayback,
    VolumeUp,
    VolumeDown,
    Rewind,
    FastForward,
    Forward15,
    Forward30,
    Back15,
    Back30,
    Next,
    Previous,
    TogglePictureInPicture,
    ExitFullscreen,
    ToggleTimeDisplay,
}

const BUTTONS: &[(Action, &str)] = &[
    (Action::TogglePlayback, "play"),
    (Action::VolumeUp, "volume-up"),
    (Action::VolumeDown, "volume-down"),
    (Action::Rewind, "rewind"),
    (Action::FastForward, "fast-forward"),
    (Action::Forward15, "forward-15"),
    (Action::Forward30, "forward-30"),
    (Action::Back15, "back-15"),
    (Action::Back30, "back-30"),
    (Action::Next, "next"),
    (Action::Previous, "previous"),
    (Action::TogglePictureInPicture, "pip"),
    (Action::ExitFullscreen, "exit-fullscreen"),
    (Action::ToggleTimeDisplay, "other-time"),
];
const PREFIX: &str = "org.riina.player-touch-bar.";

fn identifier(key: &str) -> Retained<NSString> {
    NSString::from_str(&format!("{PREFIX}{key}"))
}

#[derive(Clone, Default)]
pub(crate) struct State {
    pub(crate) loaded: bool,
    pub(crate) paused: bool,
    pub(crate) fraction: Option<f64>,
    pub(crate) elapsed: String,
    pub(crate) other: String,
}

#[derive(Default)]
struct Items {
    buttons: BTreeMap<Action, Weak<NSButton>>,
    elapsed: Weak<NSTextField>,
    scrub: Weak<NSSlider>,
}

struct DelegateState {
    input: Box<dyn Fn(Action)>,
    seek: Box<dyn Fn(f64)>,
    tracking: Rc<slider::Tracking>,
    previews: Rc<Preview>,
    captions: BTreeMap<String, String>,
    values: RefCell<State>,
    items: RefCell<Items>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateState]
    struct Delegate;

    // SAFETY: NSObject implements its own protocol.
    unsafe impl NSObjectProtocol for Delegate {}

    // SAFETY: AppKit calls this delegate on the main thread and retains its returned items.
    unsafe impl NSTouchBarDelegate for Delegate {
        #[expect(non_snake_case, reason = "Name follows the AppKit delegate trait")]
        #[unsafe(method_id(touchBar:makeItemForIdentifier:))]
        fn touchBar_makeItemForIdentifier(
            &self,
            _bar: &NSTouchBar,
            native_id: &NSTouchBarItemIdentifier,
        ) -> Option<Retained<NSTouchBarItem>> {
            self.make_item(native_id)
        }
    }

    impl Delegate {
        #[unsafe(method(riinaTouchBarButton:))]
        fn perform_button(&self, sender: &NSButton) {
            if let Some(button) = usize::try_from(sender.tag()).ok().and_then(|index| BUTTONS.get(index)) {
                (self.ivars().input)(button.0);
            }
        }

        #[unsafe(method(riinaTouchBarSeek:))]
        fn perform_seek(&self, sender: &NSSlider) {
            (self.ivars().seek)(sender.doubleValue());
            NSView::setNeedsDisplay(sender, true);
        }
    }
);

impl Delegate {
    fn caption(&self, key: &str) -> Retained<NSString> {
        NSString::from_str(self.ivars().captions.get(key).map_or("", String::as_str))
    }

    fn make_item(&self, native_id: &NSTouchBarItemIdentifier) -> Option<Retained<NSTouchBarItem>> {
        let identifier = native_id.to_string();
        let key = identifier.strip_prefix(PREFIX)?;
        let marker = self.mtm();
        if key == "scrubber" {
            let item = NSSliderTouchBarItem::initWithIdentifier(
                NSSliderTouchBarItem::alloc(marker),
                native_id,
            );
            let slider = slider::scrubber(
                marker,
                self.ivars().tracking.clone(),
                self.ivars().previews.clone(),
            );
            // SAFETY: the delegate outlives its attached bar and uses the NSSlider sender.
            unsafe {
                slider.setTarget(Some(self.as_ref()));
                slider.setAction(Some(sel!(riinaTouchBarSeek:)));
            }
            item.setSlider(&slider);
            item.setCustomizationLabel(Some(&self.caption(key)));
            self.ivars().items.borrow_mut().scrub = Weak::new(&slider);
            self.project(false);
            return Some(Retained::into_super(item));
        }

        let item = NSCustomTouchBarItem::initWithIdentifier(
            NSCustomTouchBarItem::alloc(marker),
            native_id,
        );
        let caption = self.caption(key);
        let customization_key = if key == "play" { "play-pause" } else { key };
        item.setCustomizationLabel(Some(&self.caption(customization_key)));
        if key == "elapsed" {
            let label = NSTextField::labelWithString(&NSString::from_str("00:00"), marker);
            label.setFont(Some(&clock_font()));
            item.setView(&label);
            self.ivars().items.borrow_mut().elapsed = Weak::new(&label);
        } else {
            let (index, &(action, _)) = BUTTONS
                .iter()
                .enumerate()
                .find(|(_, button)| button.1 == key)?;
            // SAFETY: the attached bar keeps the delegate alive and sends the NSButton argument.
            let button = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &caption,
                    Some(self.as_ref()),
                    Some(sel!(riinaTouchBarButton:)),
                    marker,
                )
            };
            button.setTag(isize::try_from(index).ok()?);
            if action == Action::ToggleTimeDisplay {
                button.setFont(Some(&clock_font()));
            } else if action != Action::TogglePlayback {
                set_icon(&button, action, false, &caption);
            }
            item.setView(&button);
            self.ivars()
                .items
                .borrow_mut()
                .buttons
                .insert(action, Weak::new(&button));
        }
        self.project(true);
        Some(Retained::into_super(item))
    }

    fn project(&self, update_play_symbol: bool) {
        let items = self.ivars().items.borrow();
        let state = self.ivars().values.borrow();
        for (&action, view) in &items.buttons {
            if let Some(button) = view.load() {
                button.setEnabled(state.loaded || action == Action::ExitFullscreen);
                if action == Action::TogglePlayback && update_play_symbol {
                    let title = self.caption(if state.paused { "play" } else { "pause" });
                    set_icon(&button, action, state.paused, &title);
                } else if action == Action::ToggleTimeDisplay {
                    let value = clock_text(&state.other);
                    if *button.title() != *value {
                        button.setTitle(&value);
                    }
                }
            }
        }
        if let Some(label) = items.elapsed.load() {
            let value = clock_text(&state.elapsed);
            if *label.stringValue() != *value {
                label.setStringValue(&value);
            }
        }
        if let Some(scrubber) = items.scrub.load() {
            scrubber.setEnabled(state.fraction.is_some());
            if !self.ivars().tracking.active.get() {
                scrubber.setDoubleValue(state.fraction.unwrap_or_default());
            }
        }
    }
}

fn set_icon(button: &NSButton, action: Action, paused: bool, title: &NSString) {
    button.setTitle(title);
    let image = button_icon(action, paused, title);
    button.setImage(image.as_deref());
    button.setImagePosition(if image.is_some() {
        NSCellImagePosition::ImageOnly
    } else {
        NSCellImagePosition::NoImage
    });
}

fn button_icon(action: Action, paused: bool, title: &NSString) -> Option<Retained<NSImage>> {
    // SAFETY: AppKit exports these immutable names for its built-in images.
    let name = unsafe {
        match action {
            Action::TogglePlayback if paused => kit::NSImageNameTouchBarPlayTemplate,
            Action::TogglePlayback => kit::NSImageNameTouchBarPauseTemplate,
            Action::VolumeUp => kit::NSImageNameTouchBarVolumeUpTemplate,
            Action::VolumeDown => kit::NSImageNameTouchBarVolumeDownTemplate,
            Action::Rewind => kit::NSImageNameTouchBarRewindTemplate,
            Action::FastForward => kit::NSImageNameTouchBarFastForwardTemplate,
            Action::Forward15 => kit::NSImageNameTouchBarSkipAhead15SecondsTemplate,
            Action::Forward30 => kit::NSImageNameTouchBarSkipAhead30SecondsTemplate,
            Action::Back15 => kit::NSImageNameTouchBarSkipBack15SecondsTemplate,
            Action::Back30 => kit::NSImageNameTouchBarSkipBack30SecondsTemplate,
            Action::Next => kit::NSImageNameTouchBarSkipAheadTemplate,
            Action::Previous => kit::NSImageNameTouchBarSkipBackTemplate,
            Action::ExitFullscreen => kit::NSImageNameTouchBarExitFullScreenTemplate,
            Action::TogglePictureInPicture => {
                return NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &NSString::from_str("pip.enter"),
                    Some(title),
                );
            }
            Action::ToggleTimeDisplay => return None,
        }
    };
    NSImage::imageNamed(name)
}

fn clock_font() -> Retained<NSFont> {
    // SAFETY: AppKit supplies this immutable font weight.
    NSFont::monospacedDigitSystemFontOfSize_weight(0.0, unsafe {
        objc2_app_kit::NSFontWeightRegular
    })
}

fn clock_text(value: &str) -> Retained<NSString> {
    NSString::from_str(&format!("\u{2066}{value}\u{2069}"))
}

pub(crate) struct PlayerTouchBar {
    window: Retained<NSWindow>,
    bar: Retained<NSTouchBar>,
    delegate: Retained<Delegate>,
    fullscreen_escape: Cell<bool>,
}

impl PlayerTouchBar {
    pub(crate) fn attach(
        app: &PlayerWindow,
        previews: Rc<Preview>,
        captions: BTreeMap<String, String>,
        input: impl Fn(Action) + 'static,
        seek: impl Fn(f64) + 'static,
        tracking: impl Fn(bool) + 'static,
    ) -> Option<Self> {
        let window = player_window(app)?;
        let marker = window.mtm();
        let target = Delegate::alloc(marker).set_ivars(DelegateState {
            input: Box::new(input),
            seek: Box::new(seek),
            tracking: Rc::new(slider::Tracking::new(tracking)),
            previews,
            captions,
            values: RefCell::new(State {
                paused: true,
                ..State::default()
            }),
            items: RefCell::default(),
        });
        // SAFETY: initialize the allocated, ivar-initialized NSObject on the main thread.
        let delegate: Retained<Delegate> = unsafe { objc2::msg_send![super(target), init] };
        let bar = NSTouchBar::new(marker);
        let defaults = ["play", "elapsed", "scrubber", "other-time"].map(identifier);
        // SAFETY: AppKit exports this built-in spacing identifier as an immutable string.
        let spacing = unsafe { objc2_app_kit::NSTouchBarItemIdentifierFixedSpaceLarge };
        let allowed = BUTTONS
            .iter()
            .filter(|button| button.0 != Action::ExitFullscreen)
            .map(|button| identifier(button.1))
            .chain(["elapsed", "scrubber"].map(identifier))
            .chain([NSString::from_str(&spacing.to_string())])
            .collect::<Vec<_>>();
        bar.setCustomizationIdentifier(Some(&NSString::from_str("org.riina.player")));
        bar.setDefaultItemIdentifiers(&NSArray::from_retained_slice(&defaults));
        bar.setCustomizationAllowedItemIdentifiers(&NSArray::from_retained_slice(&allowed));
        bar.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        window.setTouchBar(Some(&bar));
        Some(Self {
            window,
            bar,
            delegate,
            fullscreen_escape: Cell::new(false),
        })
    }

    pub(crate) fn refresh(&self, state: State, exit_fullscreen_on_escape: bool) {
        if self.fullscreen_escape.replace(exit_fullscreen_on_escape) != exit_fullscreen_on_escape {
            let escape = exit_fullscreen_on_escape.then(|| identifier("exit-fullscreen"));
            self.bar
                .setEscapeKeyReplacementItemIdentifier(escape.as_deref());
        }
        let previous = self.delegate.ivars().values.replace(state);
        let paused_changed = previous.paused != self.delegate.ivars().values.borrow().paused;
        self.delegate.project(paused_changed);
    }

    pub(crate) fn redraw_previews(&self) {
        if let Some(slider) = self.delegate.ivars().items.borrow().scrub.load() {
            NSView::setNeedsDisplay(&slider, true);
        }
    }
}

impl Drop for PlayerTouchBar {
    fn drop(&mut self) {
        // AppKit may still retain item views while replacing the active bar.
        let items = self.delegate.ivars().items.borrow();
        for button in items.buttons.values().filter_map(Weak::load) {
            // SAFETY: clear the target before releasing the delegate.
            unsafe { button.setTarget(None) };
        }
        if let Some(slider) = items.scrub.load() {
            // SAFETY: clear the target before releasing the delegate.
            unsafe { slider.setTarget(None) };
        }
        self.bar.setDelegate(None);
        if self
            .window
            .touchBar()
            .as_ref()
            .is_some_and(|current| Retained::as_ptr(current) == Retained::as_ptr(&self.bar))
        {
            self.window.setTouchBar(None);
        }
    }
}
