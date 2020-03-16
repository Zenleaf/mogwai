//! Elmesque components through model and view message passing.
//!
//! Sometimes an application can get so entangled that it's hard to follow the
//! path of messages through `Transmitter`s, `Receiver`s and fold functions. For
//! situations like these where complexity is unavoidable, Mogwai provides the
//! [Component] trait and the helper struct [`GizmoComponent`].
//!
//! Many rust web app libraries use a message passing pattern made famous by
//! the Elm architecture to wrangle complexity. Mogwai is similar, but different
//! - Like other libraries, messages come out of the DOM into your component's
//!   model by way of the [Component::update] function.
//! - The model is updated according to the value of the model message.
//! - _Unlike_ Elm-like libraries, view updates are sent out of the update
//!   function by hand! This sounds tedious but it's actually no big deal. You'll
//!   soon understand how easy this is in practice.
//!
//! Mogwai lacks a virtual DOM implementation. One might think that this is a
//! disadvantage but to the contrary this is a strength, as it obviates the
//! entire diffing phase of rendering DOM. This is where Mogwai gets its speed
//! advantage.
//!
//! Instead of a virtual DOM Mogwai uses one more step in its model update. The
//! `Component::update` method is given a `Transmitter<Self::ViewMsg>` with which
//! to send _view update messages_. Messages sent on this transmitter will in
//! turn be sent out to the view to update the DOM. This forms a cycle. Messages
//! come into the model from the view, update, messages go into the view from the
//! model. In this way DOM updates are obvious. You know exactly where, when and
//! why updates are made (both to the model and the view).
//!
//! Here is a minimal example of a `Component` that counts its own clicks.
//!
//! ```rust, no_run
//! extern crate mogwai;
//! use mogwai::prelude::*;
//!
//! #[derive(Clone)]
//! enum In {
//!   Click
//! }
//!
//! #[derive(Clone)]
//! enum Out {
//!   DrawClicks(i32)
//! }
//!
//! struct App {
//!   num_clicks: i32
//! }
//!
//! impl Component for App {
//!   type ModelMsg = In;
//!   type ViewMsg = Out;
//!
//!   fn builder(&self, tx: Transmitter<In>, rx:Receiver<Out>) -> GizmoBuilder {
//!     button()
//!       .tx_on("click", tx.contra_map(|_| In::Click))
//!       .rx_text("clicks = 0", rx.branch_map(|msg| {
//!         match msg {
//!           Out::DrawClicks(n) => {
//!             format!("clicks = {}", n)
//!           }
//!         }
//!       }))
//!   }
//!
//!   fn update(&mut self, msg: &In, tx_view: &Transmitter<Out>, _sub: &Subscriber<In>) {
//!     match msg {
//!       In::Click => {
//!         self.num_clicks += 1;
//!         tx_view.send(&Out::DrawClicks(self.num_clicks));
//!       }
//!     }
//!   }
//! }
//!
//!
//! pub fn main() -> Result<(), JsValue> {
//!   App{ num_clicks: 0 }
//!   .into_component()
//!   .run()
//! }
//! ```
//!
//! The first step is to define the incoming messages that will update the model.
//! Next we define the outgoing messages that will update our view. The `builder`
//! trait method uses these message types to build the view. It does this by
//! consuming a `Transmitter<Self::ModelMsg>` and a `Receiver<Self::ViewMsg>`.
//! These represent the inputs and the outputs of your component. Roughly,
//! `Self::ModelMsg` comes into the `update` function and `Self::ViewMsg`s go out
//! of the `update` function.
//!
//! ## Communicating to components
//!
//! If your component is owned by another, the parent component can communicate to
//! the child through its messages, either by calling [`GizmoComponent::update`]
//! on the child component within its own `update` function or by subscribing to
//! the child component's messages when the child component is created (see
//! [`Subscriber`]).
//!
//! ## Placing components
//!
//! Components may be used within a [`GizmoBuilder`] using the
//! [`GizmoBuilder::with`] function.
use std::any::Any;
use std::sync::{Arc, Mutex};
use std::ops::Deref;
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::Node;

use super::gizmo::{Gizmo, SubGizmo};
use super::txrx::{txrx, Receiver, Transmitter};
use super::utils;

pub mod subscriber;
use subscriber::Subscriber;


/// Defines a component with distinct input (model update) and output
/// (view update) messages.
///
/// See the [module level documentation](super::component) for more details.
pub trait Component
where
  Self: Any + Sized,
  Self::ModelMsg: Clone,
  Self::ViewMsg: Clone,
  Self::DomNode: JsCast + AsRef<Node> + Clone,
{
  /// A model message comes out from the view through a tx_on function into your
  /// component's update function.
  type ModelMsg;

  /// A view message comes out from your component's update function and changes
  /// the view by being used in an rx_* function.
  type ViewMsg;

  /// The type of DOM node that represents the root of this component.
  type DomNode;

  /// Update this component in response to any received model messages.
  /// This is essentially the component's fold function.
  fn update(
    &mut self,
    msg: &Self::ModelMsg,
    tx_view: &Transmitter<Self::ViewMsg>,
    sub: &Subscriber<Self::ModelMsg>,
  );

  /// Produce this component's gizmo using inputs and outputs.
  fn view(
    &self,
    tx: Transmitter<Self::ModelMsg>,
    rx: Receiver<Self::ViewMsg>,
  ) -> Gizmo<Self::DomNode>;

  /// Helper function for constructing a GizmoComponent for a type that
  /// implements Component.
  fn into_component(self) -> GizmoComponent<Self> {
    GizmoComponent::new(self)
  }
}


impl<T, D> From<T> for Gizmo<D>
where
  T: Component,
  T::DomNode: AsRef<D>,
  D: JsCast + 'static
{
  fn from(component: T) -> Gizmo<D> {
    let gizmo:Gizmo<T::DomNode> =
    component
      .into_component()
      .gizmo;
    gizmo.upcast::<D>()
  }
}


impl<T> SubGizmo for T
  where
  T: Component,
  T::DomNode: AsRef<Node>
{
  fn into_sub_gizmo(self) -> Result<Gizmo<Node>, Node> {
    let component:GizmoComponent<T> = self.into_component();
    component.into_sub_gizmo()
  }
}


/// A component and all of its pieces.
///
/// TODO: Think about renaming Gizmo to Dom and GizmoComponent to Gizmo.
/// I think people will use this GizmoComponent more often.
pub struct GizmoComponent<T: Component> {
  pub trns: Transmitter<T::ModelMsg>,
  pub recv: Receiver<T::ViewMsg>,

  pub(crate) gizmo: Gizmo<T::DomNode>,
  pub(crate) state: Arc<Mutex<T>>,
}


impl<T:Component> Deref for GizmoComponent<T> {
  type Target = Gizmo<T::DomNode>;

  fn deref(&self) -> &Gizmo<T::DomNode> {
    self.gizmo_ref()
  }
}


impl<T> GizmoComponent<T>
where
  T: Component + 'static,
  T::ViewMsg: Clone,
  T::DomNode: AsRef<Node> + Clone
{
  pub fn new(init: T) -> GizmoComponent<T> {
    let component_var = Arc::new(Mutex::new(init));
    let state = component_var.clone();
    let (tx_out, rx_out) = txrx();
    let (tx_in, rx_in) = txrx();
    let subscriber = Subscriber::new(&tx_in);

    let (tx_view, rx_view) = txrx();
    rx_in.respond(move |msg: &T::ModelMsg| {
      let mut t =
        state
        .try_lock()
        .expect("Could not get component state lock");
      T::update(&mut t, msg, &tx_view, &subscriber);
    });

    let out_msgs = Arc::new(Mutex::new(vec![]));
    rx_view.respond(move |msg: &T::ViewMsg| {
      let should_schedule =
        {
        let mut msgs =
          out_msgs
          .try_lock()
          .expect("Could not try_lock to push to out_msgs");
        msgs.push(msg.clone());
        // If there is more than just this message in the queue, this
        // responder has already been run this frame and a timer has
        // already been scheduled, so there's no need to schedule another
        msgs.len() == 1
      };
      if should_schedule {
        let out_msgs_async = out_msgs.clone();
        let tx_out_async = tx_out.clone();
        utils::timeout(0, move || {
          let msgs =
            {
            out_msgs_async
              .try_lock()
              .expect("Could not try_lock to pop out_msgs")
              .drain(0..)
              .collect::<Vec<_>>()
          };
          if msgs.len() > 0 {
            msgs.iter().for_each(|out_msg| {
              tx_out_async.send(out_msg);
            });
          }
          false
        });
      }
    });

    let gizmo = {
      let component = component_var.try_lock().unwrap_throw();
      component.view(tx_in.clone(), rx_out.branch())
    };

    GizmoComponent {
      trns: tx_in,
      recv: rx_out,
      gizmo,
      state: component_var,
    }
  }

  /// A reference to the DomNode.
  pub fn dom_ref(&self) -> &T::DomNode {
    let gizmo:&Gizmo<T::DomNode> = &self.gizmo;
    gizmo.element.unchecked_ref()
  }

  /// A reference to the Gizmo.
  pub fn gizmo_ref(&self) -> &Gizmo<T::DomNode> {
    &self.gizmo
  }

  /// Send model messages into this component from a `Receiver<T::ModelMsg>`.
  /// This is helpful for sending messages to this component from
  /// a parent component.
  pub fn rx_from(self, rx: Receiver<T::ModelMsg>) -> GizmoComponent<T> {
    rx.forward_map(&self.trns, |msg| msg.clone());
    self
  }

  /// Send view messages from this component into a `Transmitter<T::ViewMsg>`.
  /// This is helpful for sending messages to this component from
  /// a parent component.
  pub fn tx_into(self, tx: &Transmitter<T::ViewMsg>) -> GizmoComponent<T> {
    self.recv.branch().forward_map(&tx, |msg| msg.clone());
    self
  }

  /// Run and initialize the component with a list of messages.
  /// This is equivalent to calling `run` and `update` with each message.
  pub fn run_init(mut self, msgs: Vec<T::ModelMsg>) -> Result<(), JsValue> {
    msgs.into_iter().for_each(|msg| {
      self.update(&msg);
    });
    self.run()
  }

  /// Run this component forever
  pub fn run(self) -> Result<(), JsValue> {
    self.gizmo.run()
  }

  /// Append this component's gizmo an HtmlElement.
  /// Has no effect if this component has not been built.
  pub fn append_to(&self, parent: &Node) {
    parent
      .append_child(self.gizmo.as_ref())
      .expect("could not append component to parent node");
  }

  /// Update the component with the given message.
  /// This how a parent component communicates down to its child components.
  pub fn update(&mut self, msg: &T::ModelMsg) {
    self.trns.send(msg);
  }

  /// Access the component's underlying state.
  pub fn with_state<F, N>(&self, f: F) -> N
  where
    F: Fn(&T) -> N,
  {
    let t =
      self
      .state
      .try_lock()
      .expect("Could not get lock on GizmoComponent state");
    f(&t)
  }
}


impl<T> SubGizmo for GizmoComponent<T>
where
  T: Component,
  T::DomNode: AsRef<Node>
{
  fn into_sub_gizmo(self) -> Result<Gizmo<Node>, Node> {
    self.gizmo.into_sub_gizmo()
  }
}


/// The type of function that uses a txrx pair and returns a Gizmo.
pub type BuilderFn<T, D> = dyn Fn(Transmitter<T>, Receiver<T>) -> Gizmo<D>;


/// A simple component made from a [BuilderFn].
///
/// Any function that takes a transmitter and receiver of the same type and
/// returns a GizmoBuilder can be made into a component that holds no internal
/// state. It forwards all of its incoming messages to its view.
///
/// ```rust
/// extern crate mogwai;
/// use mogwai::prelude::*;
///
/// let component: SimpleComponent<()> =
///   (Box::new(
///   |tx: Transmitter<()>, rx: Receiver<()>| -> GizmoBuilder {
///     button()
///       .style("cursor", "pointer")
///       .rx_text("Click me", rx.branch_map(|()| "Clicked!".to_string()))
///       .tx_on("click", tx.contra_map(|_| ()))
///   },
/// ) as Box<BuilderFn<()>>)
///   .into_component();
/// ```
pub type SimpleComponent<T, D> = GizmoComponent<Box<BuilderFn<T, D>>>;


impl<T, D> Component for Box<BuilderFn<T, D>>
where
  T: Any + Clone,
  D: JsCast + AsRef<Node> + Clone + 'static
{
  type ModelMsg = T;
  type ViewMsg = T;
  type DomNode = D;

  fn update(
    &mut self,
    msg: &T,
    tx_view: &Transmitter<T>,
    _sub: &Subscriber<T>,
  ) {
    tx_view.send(msg);
  }

  fn view(&self, tx: Transmitter<T>, rx: Receiver<T>) -> Gizmo<D> {
    self(tx, rx)
  }
}
