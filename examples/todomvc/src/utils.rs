use web_sys::{Event, HtmlElement, HtmlInputElement};
use wasm_bindgen::JsCast;


pub fn input_value(input:&HtmlElement) -> Option<String> {
  let input:HtmlInputElement =
    input
    .clone()
    .dyn_into()
    .ok()?;
  Some(
    input
      .value()
      .trim()
      .to_string()
  )
}


pub fn event_input_value(ev:&Event) -> Option<String> {
  let input:HtmlInputElement =
    ev
    .target()?
    .dyn_into()
    .ok()?;
  Some(
    input
      .value()
      .trim()
      .to_string()
  )
}
