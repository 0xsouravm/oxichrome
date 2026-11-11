use wasm_bindgen::JsValue;

use crate::error::{OxichromeError, Result};

#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be an event handler argument",
    label = "unsupported argument type",
    note = "use `JsValue`, `String`, `bool`, `u32`, `i32`, an `Option` of those, or a type oxichrome provides for this event such as `alarms::Alarm`, `tabs::Tab` or `windows::Window`"
)]
pub trait FromEventArg: Sized {
    fn from_event_arg(value: JsValue) -> Result<Self>;
}

impl FromEventArg for JsValue {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        Ok(value)
    }
}

impl FromEventArg for String {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        value.as_string().ok_or_else(|| {
            OxichromeError::UnexpectedValue(format!("expected a string, got {value:?}"))
        })
    }
}

impl FromEventArg for bool {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        value.as_bool().ok_or_else(|| {
            OxichromeError::UnexpectedValue(format!("expected a boolean, got {value:?}"))
        })
    }
}

impl FromEventArg for u32 {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        match value.as_f64() {
            Some(number) if number.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&number) => {
                Ok(number as u32)
            }
            _ => Err(OxichromeError::UnexpectedValue(format!(
                "expected a whole number from 0 to {}, got {value:?}",
                u32::MAX
            ))),
        }
    }
}

impl FromEventArg for i32 {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        match value.as_f64() {
            Some(number) if number.fract() == 0.0 && (i32::MIN as f64..=i32::MAX as f64).contains(&number) => {
                Ok(number as i32)
            }
            _ => Err(OxichromeError::UnexpectedValue(format!("expected a whole number, got {value:?}"))),
        }
    }
}

impl<T: FromEventArg> FromEventArg for Option<T> {
    fn from_event_arg(value: JsValue) -> Result<Self> {
        if value.is_undefined() || value.is_null() {
            return Ok(None);
        }
        T::from_event_arg(value).map(Some)
    }
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be argument {POSITION} of this event handler",
    label = "wrong type for this event",
    note = "`JsValue` always works; see the event's documentation for the typed argument it takes"
)]
pub trait EventArg<Event, const POSITION: usize>: FromEventArg {}

impl<Event, const POSITION: usize> EventArg<Event, POSITION> for JsValue {}

#[doc(hidden)]
pub fn assert_arg<T: EventArg<Event, POSITION>, Event, const POSITION: usize>() {}

pub trait Event {
    const ARGS: usize;
}

pub mod events {
    pub struct RuntimeOnInstalled;
    pub struct RuntimeOnMessage;
    pub struct StorageOnChanged;
    pub struct TabsOnUpdated;
    pub struct TabsOnRemoved;
    pub struct TabsOnActivated;
    pub struct ActionOnClicked;
    pub struct AlarmsOnAlarm;
    pub struct NotificationsOnClicked;
    pub struct NotificationsOnClosed;
    pub struct NotificationsOnButtonClicked;
    pub struct ContextMenusOnClicked;
    pub struct CommandsOnCommand;
    pub struct PermissionsOnAdded;
    pub struct PermissionsOnRemoved;
    pub struct WindowsOnCreated;
    pub struct WindowsOnRemoved;
    pub struct WindowsOnFocusChanged;
    pub struct WindowsOnBoundsChanged;
    pub struct SidePanelOnOpened;
    pub struct SidePanelOnClosed;
}

macro_rules! arity {
    ($($event:ident = $args:literal),* $(,)?) => {
        $(impl Event for events::$event {
            const ARGS: usize = $args;
        })*
    };
}

arity! {
    RuntimeOnInstalled = 1,
    RuntimeOnMessage = 3,
    StorageOnChanged = 2,
    TabsOnUpdated = 3,
    TabsOnRemoved = 2,
    TabsOnActivated = 1,
    ActionOnClicked = 1,
    AlarmsOnAlarm = 1,
    NotificationsOnClicked = 1,
    NotificationsOnClosed = 2,
    NotificationsOnButtonClicked = 2,
    ContextMenusOnClicked = 2,
    CommandsOnCommand = 2,
    PermissionsOnAdded = 1,
    PermissionsOnRemoved = 1,
    WindowsOnCreated = 1,
    WindowsOnRemoved = 1,
    WindowsOnFocusChanged = 1,
    WindowsOnBoundsChanged = 1,
    SidePanelOnOpened = 1,
    SidePanelOnClosed = 1,
}

impl EventArg<events::NotificationsOnClicked, 1> for String {}
impl EventArg<events::NotificationsOnClosed, 1> for String {}
impl EventArg<events::NotificationsOnClosed, 2> for Option<bool> {}
impl EventArg<events::NotificationsOnButtonClicked, 1> for String {}
impl EventArg<events::NotificationsOnButtonClicked, 2> for u32 {}
