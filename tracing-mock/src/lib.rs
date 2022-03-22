extern crate self as tracking_mock;

pub mod event;
pub mod fields;
pub mod metadata;
pub mod span;
pub mod subscribe;

pub use self::{
    event::MockEvent, fields::MockFields, metadata::MockMetadata, span::MockSpan,
    subscribe::MockSubscribe,
};

#[macro_export]
macro_rules! expect {
    // NewSpan
    [[$($done:tt)*]; NewSpan($span:expr, $fields:expr) {$field:ident: $value:expr $(, $($rest:tt)*)?} $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)*]; NewSpan($span.$field($value.into()), $fields) {$($($rest)*)?} $(, $($tt:tt)*)? }
    };
    [[$($done:tt)*]; NewSpan($span:expr, $fields:expr) {; $field:ident: $value:expr $(, $($rest:tt)*)?} $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)*]; NewSpan($span, $fields.$field($value.into())) {; $($($rest)*)?} $(, $($tt:tt)*)? }
    };
    [[$($done:tt)*]; NewSpan($span:expr, $fields:expr) {;} $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)*]; NewSpan($span, $fields) {} $(, $($tt:tt)*)? }
    };
    [[$($done:tt)*]; NewSpan($span:expr, $fields:expr) {} $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)* $crate::subscribe::Expect::NewSpan($span, $fields),]; $(, $($tt)*)? }
    };
    [[$($done:tt)*]; NewSpan $({$($rest:tt)*})? $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)*]; NewSpan($crate::span::mock(), $crate::fields::mock()) {$($($rest)*)?} $($tt:tt)* }
    };

    // Event
    [[$($done:tt)*]; Event($event:expr) {$field:ident: $value:expr $(, $($rest:tt)*)?} $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)*]; Event($event.$field($value.into())) {$($($rest)*)?} $(, $($tt:tt)*)? }
    };
    [[$($done:tt)*]; Event($event:expr) {} $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)* $crate::subscribe::Expect::Event($event),]; $(, $($tt)*)? }
    };
    [[$($done:tt)*]; Event $({$($rest:tt)*})? $(, $($tt:tt)*)?] => {
        $crate::expect! { [$($done)*]; Event($crate::event::mock()) {$($($rest)*)?} $($tt:tt)* }
    };

    // Exit
    [[$($done:tt)*];] => { [$($done)*] };

    // Whoops
    [[$($done:tt)*]; $($rest:tt)+] => { $crate::unexpected_token! { $($rest)+ } };

    // Entry
    [$($tt:tt)*]      => { $crate::expect! { []; $($tt)* } };
}

#[doc(hidden)]
#[macro_export]
macro_rules! unexpected_token {
    (__if_you_use_this_token_name_and_get_a_worse_error_message_its_on_you) => {};
}
