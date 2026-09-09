use proxmox_yew_comp::{EditWindow, EditWindowLayout, Wizard};
use pwt::css::FlexFit;
use pwt::prelude::*;
use pwt::widget::form::{Field, FormContext};
use pwt::widget::{Button, Column, Container, Dialog, Row, TabBarItem};

fn query() -> String {
    gloo_utils::window().location().search().unwrap()
}

fn record(event: &str) {
    let output = gloo_utils::document().get_element_by_id("events").unwrap();
    let text = output.text_content().unwrap_or_default();
    output.set_text_content(Some(&format!("{text}{event}\n")));
}

#[function_component(Editor)]
fn editor() -> Html {
    let open = use_state(|| true);
    let confirm = use_state(|| false);
    let options = query();
    let accept = options.contains("accept");
    let prompt = options.contains("confirm");
    let fail = options.contains("failure");
    let check = {
        let confirm = confirm.clone();
        Callback::from(move |form: FormContext| {
            record(&format!("check:{}", form.read().get_field_text("value")));
            if prompt {
                confirm.set(true);
            }
            accept
        })
    };
    let done = {
        let open = open.clone();
        move |_| {
            record("done");
            open.set(false);
        }
    };
    let editor = (*open).then(|| {
        EditWindow::new("Editor")
            .layout(if options.contains("adaptive") {
                EditWindowLayout::Adaptive
            } else {
                EditWindowLayout::Dialog
            })
            .width("min(40rem, 94vw)")
            .height("min(34rem, 90dvh)")
            .max_height("90dvh")
            .inline_error(!options.contains("alert"))
            .before_close((!options.contains("unguarded")).then_some(check))
            .on_close(|_| record("close"))
            .on_done(done)
            .submit_text("Save")
            .on_submit(move |_| async move {
                record("submit");
                if fail {
                    anyhow::bail!("Save failed. The server rejected this change. Correct the retained inputs and retry without leaving the editor.");
                }
                Ok(())
            })
            .renderer(|_| {
                let mut body = Column::new().class(FlexFit).padding(2).with_child(
                    Field::new().name("value").default("original").aria_label("Value"),
                );
                for index in 1..=30 {
                    body.add_child(
                        Container::new().with_child(format!("Scrollable form content {index}")),
                    );
                }
                body.into()
            })
    });
    let confirmation = (*confirm).then(|| {
        let cancel = {
            let confirm = confirm.clone();
            Callback::from(move |_| confirm.set(false))
        };
        let discard = {
            let open = open.clone();
            let confirm = confirm.clone();
            move |_| {
                record("discard");
                confirm.set(false);
                open.set(false);
            }
        };
        Dialog::new("Discard changes?")
            .on_close(cancel.clone())
            .with_child(
                Row::new()
                    .padding(2)
                    .gap(2)
                    .with_child(Button::new("Keep editing").onclick(move |_| cancel.emit(())))
                    .with_child(Button::new("Discard").onclick(discard)),
            )
    });
    Container::new()
        .with_child(Button::new("Outside").attribute("id", "outside"))
        .with_child(Container::from_tag("pre").attribute("id", "events"))
        .with_child(editor.map(Html::from).unwrap_or_default())
        .with_child(confirmation.map(Html::from).unwrap_or_default())
        .into()
}

#[function_component(WizardEditor)]
fn wizard_editor() -> Html {
    let open = use_state(|| true);
    let confirm = use_state(|| false);
    let identity = use_mut_ref(|| None::<FormContext>);
    let check = {
        let identity = identity.clone();
        let confirm = confirm.clone();
        Callback::from(move |()| {
            let value = identity
                .borrow()
                .as_ref()
                .map(|form| form.read().get_field_text("identity"))
                .unwrap_or_default();
            record(&format!("check:{value}"));
            if value.trim().is_empty() {
                true
            } else {
                confirm.set(true);
                false
            }
        })
    };
    let done = {
        let open = open.clone();
        move |_| {
            record("done");
            open.set(false);
        }
    };
    let wizard = (*open).then(|| {
        Wizard::new("Wizard")
            .width("min(40rem, 94vw)")
            .max_height("90dvh")
            .before_close(check)
            .on_close(|_| record("close"))
            .on_done(done)
            .with_page(
                TabBarItem::new().key("identity").label("Identity"),
                move |info| {
                    *identity.borrow_mut() = Some(info.form_ctx.clone());
                    Column::new()
                        .class(FlexFit)
                        .padding(2)
                        .with_child(Field::new().name("identity").required(true))
                        .into()
                },
            )
            .with_page(TabBarItem::new().key("details").label("Details"), |_| {
                Column::new()
                    .class(FlexFit)
                    .padding(2)
                    .with_child(Field::new().name("details"))
                    .into()
            })
            .submit_text("Finish")
            .on_submit(|_| async {
                record("submit");
                Ok::<(), anyhow::Error>(())
            })
    });
    let confirmation = (*confirm).then(|| {
        let cancel = {
            let confirm = confirm.clone();
            Callback::from(move |_| confirm.set(false))
        };
        let discard = {
            let open = open.clone();
            let confirm = confirm.clone();
            move |_| {
                record("discard");
                confirm.set(false);
                open.set(false);
            }
        };
        Dialog::new("Discard changes?")
            .on_close(cancel.clone())
            .with_child(
                Row::new()
                    .padding(2)
                    .gap(2)
                    .with_child(Button::new("Keep editing").onclick(move |_| cancel.emit(())))
                    .with_child(Button::new("Discard").onclick(discard)),
            )
    });
    Container::new()
        .with_child(Button::new("Outside").attribute("id", "outside"))
        .with_child(Container::from_tag("pre").attribute("id", "events"))
        .with_child(wizard.map(Html::from).unwrap_or_default())
        .with_child(confirmation.map(Html::from).unwrap_or_default())
        .into()
}

#[function_component(Growth)]
fn growth() -> Html {
    let extent = use_state(|| 160);
    let grow = {
        let extent = extent.clone();
        move |_| extent.set(600)
    };
    let shrink = {
        let extent = extent.clone();
        move |_| extent.set(160)
    };
    Dialog::new("Growth")
        .on_close(|_| ())
        .auto_center(!query().contains("manual"))
        .resizable(true)
        .width(350)
        .with_child(
            Column::new()
                .with_child(Button::new("Grow").onclick(grow))
                .with_child(Button::new("Shrink").onclick(shrink))
                .with_child(Container::new().height(*extent)),
        )
        .into()
}

#[function_component(App)]
fn app() -> Html {
    if query().contains("growth") {
        html! { <Growth /> }
    } else if query().contains("wizard") {
        html! { <WizardEditor /> }
    } else {
        html! { <Editor /> }
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
