use yew::html::{IntoEventCallback, IntoPropValue};

use pwt::widget::{Button, Column, MessageBoxButtons};
use pwt::{prelude::*, widget::MessageBox};

use pwt_macros::{builder, widget};

pub fn default_confirm_remove_message(name: Option<impl std::fmt::Display>) -> String {
    match name {
        Some(name) => tr!("Are you sure you want to remove entry {0}", name),
        None => tr!("Are you sure you want to remove this entry?"),
    }
}

#[widget(comp=ProxmoxConfirmButton)]
#[derive(Properties, PartialEq, Clone)]
#[builder]
pub struct ConfirmButton {
    /// Button text.
    #[prop_or_default]
    pub text: Option<AttrValue>,

    /// Icon (CSS class).
    #[prop_or_default]
    pub icon_class: Option<Classes>,

    /// Html tabindex attribute.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub tabindex: Option<i32>,

    /// ARIA label.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub aria_label: Option<AttrValue>,

    /// Html placeholder attribute.
    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub placeholder: Option<AttrValue>,

    /// Html autofocus attribute.
    #[prop_or_default]
    #[builder]
    pub autofocus: bool,

    /// Disable flag.
    #[prop_or_default]
    #[builder]
    pub disabled: bool,

    /// Activate callback (called after confirm)
    #[prop_or_default]
    #[builder_cb(IntoEventCallback, into_event_callback, ())]
    pub on_activate: Option<Callback<()>>,

    #[prop_or_default]
    #[builder(IntoPropValue, into_prop_value)]
    pub confirm_message: Option<Html>,
}

impl ConfirmButton {
    /// Create a new button.
    pub fn new(text: impl IntoPropValue<Option<AttrValue>>) -> Self {
        yew::props!(Self {
            text: text.into_prop_value()
        })
    }

    /// Create a new icon button (without text).
    pub fn new_icon(icon_class: impl Into<Classes>) -> Self {
        yew::props!(Self {}).icon_class(icon_class)
    }

    /// Create a standard remove button.
    pub fn remove_entry(name: impl IntoPropValue<Option<AttrValue>>) -> Self {
        let name = name.into_prop_value();
        let message = default_confirm_remove_message(name.as_deref());
        yew::props!(Self {
            text: tr!("Remove"),
            confirm_message: html! {message},
        })
    }

    /// Builder style method to set the icon CSS class.
    pub fn icon_class(mut self, icon_class: impl Into<Classes>) -> Self {
        self.set_icon_class(icon_class);
        self
    }

    /// Method to set the icon CSS class.
    pub fn set_icon_class(&mut self, icon_class: impl Into<Classes>) {
        self.icon_class = Some(icon_class.into());
    }
}

pub enum Msg {
    Request,
    Activate,
    CloseDialog,
}
#[doc(hidden)]
pub struct ProxmoxConfirmButton {
    dialog: Option<Html>,
}

impl Component for ProxmoxConfirmButton {
    type Message = Msg;
    type Properties = ConfirmButton;

    fn create(_ctx: &Context<Self>) -> Self {
        Self { dialog: None }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();
        match msg {
            Msg::Request => {
                if let Some(message) = &props.confirm_message {
                    if self.dialog.is_some() {
                        return false;
                    }

                    let dialog = MessageBox::new(tr!("Confirm"), message.clone())
                        .buttons(MessageBoxButtons::YesNo)
                        .on_close(ctx.link().callback(|confirm| {
                            if confirm {
                                Msg::Activate
                            } else {
                                Msg::CloseDialog
                            }
                        }));

                    self.dialog = Some(dialog.into());
                } else {
                    ctx.link().send_message(Msg::Activate);
                }
                true
            }
            Msg::Activate => {
                self.dialog = None;
                if let Some(on_activate) = &props.on_activate {
                    on_activate.emit(());
                }
                false
            }
            Msg::CloseDialog => {
                self.dialog = None;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        let button = Button::new(props.text.clone())
            .with_std_props(&props.std_props)
            .icon_class(props.icon_class.clone())
            .tabindex(props.tabindex)
            .aria_label(props.aria_label.clone())
            .placeholder(props.placeholder.clone())
            .autofocus(props.autofocus)
            .disabled(props.disabled)
            .onclick(ctx.link().callback(|_| Msg::Request));

        Column::new()
            .class("pwt-flex-fill-first-child")
            .with_child(button)
            .with_optional_child(self.dialog.clone())
            .into()
    }
}
