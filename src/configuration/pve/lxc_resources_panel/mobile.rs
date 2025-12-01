use std::ops::{Deref, DerefMut};

use serde_json::Value;

use yew::prelude::*;

use pwt::prelude::*;
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::{Container, Fa, List, ListTile};

use pwt::props::{IntoOptionalInlineHtml, SubmitCallback};

use pve_api_types::{LxcConfig, LxcConfigMpArray, LxcConfigUnusedArray};

use crate::configuration::pve::guest::{confirm_delete_volume, confirm_detach_entry};
use crate::configuration::{guest_config_url, guest_pending_url};
use crate::form::pve::{
    lxc_cores_property, lxc_memory_property, lxc_mount_point_property, lxc_rootfs_property,
    lxc_swap_property, lxc_unused_volume_property, PveGuestType,
};
use crate::form::typed_load;
use crate::pending_property_view::{
    pending_typed_load, PendingPropertyList, PendingPropertyView, PendingPropertyViewMsg,
    PendingPropertyViewState, PvePendingConfiguration, PvePendingPropertyView,
};
use crate::EditableProperty;

use super::{is_unprivileged, EditAction, LxcResourcesPanel, Msg};
use crate::layout::card::standard_card;

pub struct PveLxcResourcesPanel {
    view_state: PendingPropertyViewState,
}

impl Deref for PveLxcResourcesPanel {
    type Target = PendingPropertyViewState;

    fn deref(&self) -> &Self::Target {
        &self.view_state
    }
}

impl DerefMut for PveLxcResourcesPanel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.view_state
    }
}

type PveLxcResourcesPanelContext = Context<PvePendingPropertyView<PveLxcResourcesPanel>>;

impl PveLxcResourcesPanel {
    fn property_tile(
        &self,
        ctx: &PveLxcResourcesPanelContext,
        current: &Value,
        pending: &Value,
        property: EditableProperty,
        icon: Fa,
        trailing: impl IntoOptionalInlineHtml,
        edit_action: EditAction,
    ) -> ListTile {
        let props = ctx.props();

        let on_revert = (!props.readonly).then(|| {
            Callback::from({
                let property = property.clone();
                ctx.link().callback(move |_: Event| {
                    PendingPropertyViewMsg::RevertProperty(property.clone())
                })
            })
        });

        let mut list_tile = PendingPropertyList::render_icon_list_tile(
            current, pending, &property, icon, trailing, on_revert,
        );

        if !props.readonly {
            match edit_action {
                //EditAction::None => { /* do nothing  */ }
                EditAction::Add | EditAction::Edit => {
                    list_tile.set_interactive(true);
                    list_tile.set_on_activate(ctx.link().callback({
                        let property = property.clone();
                        move |_| {
                            if edit_action == EditAction::Edit {
                                PendingPropertyViewMsg::EditProperty(property.clone(), None)
                            } else {
                                PendingPropertyViewMsg::AddProperty(property.clone(), None)
                            }
                        }
                    }));
                }
            }
        }

        list_tile
    }

    fn property_tile_with_menu(
        &self,
        ctx: &PveLxcResourcesPanelContext,
        current: &Value,
        pending: &Value,
        property: EditableProperty,
        icon: Fa,
        menu: Menu,
        edit_action: EditAction,
    ) -> ListTile {
        let props = ctx.props();

        let menu_button: Html = MenuButton::new("")
            .class(pwt::css::ColorScheme::Neutral)
            .class("circle")
            .icon_class("fa fa-ellipsis-v fa-lg")
            .menu(menu)
            .into();
        self.property_tile(
            ctx,
            current,
            pending,
            property,
            icon,
            if props.readonly {
                html! {}
            } else {
                menu_button
            },
            edit_action,
        )
    }

    fn disk_menu(
        &self,
        ctx: &PveLxcResourcesPanelContext,
        name: &str,
        with_move: bool,
        with_reassign: bool,
        with_resize: bool,
    ) -> Menu {
        let mut menu = Menu::new();

        if with_move {
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Move Storage"))
                    .icon_class("fa fa-database")
                    .on_select(ctx.link().callback(move |_| {
                        PendingPropertyViewMsg::Custom(Msg::MoveDisk(name.clone()))
                    }))
            });
        }

        if with_reassign {
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Reassign Owner"))
                    .icon_class("fa fa-desktop")
                    .on_select(ctx.link().callback(move |_| {
                        PendingPropertyViewMsg::Custom(Msg::ReassignDisk(name.clone()))
                    }))
            });
        }

        if with_resize {
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Resize"))
                    .icon_class("fa fa-plus")
                    .on_select(ctx.link().callback(move |_| {
                        PendingPropertyViewMsg::Custom(Msg::ResizeDisk(name.clone()))
                    }))
            });
        }
        menu
    }

    fn disk_list_tile(
        &self,
        ctx: &PveLxcResourcesPanelContext,
        name: &str,
        record: &Value,
        pending: &Value,
        unprivileged: bool,
    ) -> ListTile {
        let props = ctx.props();
        let mut menu = self.disk_menu(ctx, name, true, true, true);

        let property = lxc_mount_point_property(
            Some(name.to_string()),
            Some(props.node.clone()),
            props.remote.clone(),
            unprivileged,
            true,
        );

        let icon = Fa::new("hdd-o");

        menu.add_item({
            let link = ctx.link().clone();
            let dialog: Html = confirm_detach_entry(name, true)
                .on_close(link.callback(|_| PendingPropertyViewMsg::ShowDialog(None)))
                .on_confirm(link.callback({
                    let name = name.to_string();
                    move |_| PendingPropertyViewMsg::Delete(name.clone(), None)
                }))
                .into();

            MenuItem::new(tr!("Detach"))
                .icon_class("fa fa-sign-out")
                .on_select(
                    ctx.link().callback(move |_| {
                        PendingPropertyViewMsg::ShowDialog(Some(dialog.clone()))
                    }),
                )
        });

        let mut tile = self.property_tile_with_menu(
            ctx,
            record,
            pending,
            property,
            icon,
            menu,
            EditAction::Edit,
        );
        tile.set_key(name.to_string());
        tile
    }

    fn unused_disk_list_tile(
        &self,
        ctx: &PveLxcResourcesPanelContext,
        name: &str,
        record: &Value,
        pending: &Value,
        unprivileged: bool,
    ) -> ListTile {
        let props = ctx.props();
        let menu =
            self.disk_menu(ctx, name, false, true, false).with_item({
                let link = ctx.link().clone();
                let volume = record[name].as_str().unwrap_or(&name);
                let dialog: Html = confirm_delete_volume(name, volume, true)
                    .on_close(link.callback(|_| PendingPropertyViewMsg::ShowDialog(None)))
                    .on_confirm(link.callback({
                        let name = name.to_string();
                        move |_| PendingPropertyViewMsg::Delete(name.clone(), None)
                    }))
                    .into();

                MenuItem::new(tr!("Delete volume"))
                    .icon_class("fa fa-trash-o")
                    .on_select(ctx.link().callback(move |_| {
                        PendingPropertyViewMsg::ShowDialog(Some(dialog.clone()))
                    }))
            });

        let icon = Fa::new("hdd-o");
        let property = lxc_unused_volume_property(
            name.to_string(),
            Some(props.node.clone()),
            props.remote.clone(),
            unprivileged,
            true,
        );

        let mut tile = self.property_tile_with_menu(
            ctx,
            record,
            pending,
            property,
            icon,
            menu,
            EditAction::Add,
        );
        tile.set_key(name.to_string());
        tile
    }

    fn view_list(&self, ctx: &PveLxcResourcesPanelContext, data: &PvePendingConfiguration) -> Html {
        let props = ctx.props();
        let mut list: Vec<ListTile> = Vec::new();

        let unprivileged = is_unprivileged(data);

        let PvePendingConfiguration {
            current,
            pending,
            keys,
        } = data;

        let push_property_tile = |list: &mut Vec<_>, property: EditableProperty, icon, editable| {
            let name = match property.get_name() {
                Some(name) => name.to_string(),
                None::<_> => return,
            };

            if property.required || keys.contains(&name) {
                let mut tile =
                    self.property_tile(ctx, current, pending, property, icon, (), editable);
                tile.set_key(name);
                list.push(tile);
            }
        };

        push_property_tile(
            &mut list,
            lxc_memory_property(true),
            Fa::new("memory"),
            EditAction::Edit,
        );

        push_property_tile(
            &mut list,
            lxc_swap_property(true),
            Fa::new("retweet"),
            EditAction::Edit,
        );

        push_property_tile(
            &mut list,
            lxc_cores_property(true),
            Fa::new("cpu"),
            EditAction::Edit,
        );

        push_property_tile(
            &mut list,
            lxc_rootfs_property(
                Some(props.node.clone()),
                props.remote.clone(),
                unprivileged,
                true,
            ),
            Fa::new("hdd-o"),
            EditAction::Edit,
        );

        for n in 0..LxcConfigMpArray::MAX {
            let name = format!("mp{n}");
            if !keys.contains(&name) {
                continue;
            }
            list.push(self.disk_list_tile(ctx, &name, current, pending, unprivileged));
        }

        for n in 0..LxcConfigUnusedArray::MAX {
            let name = format!("unused{n}");
            if !keys.contains(&name) {
                continue;
            }
            list.push(self.unused_disk_list_tile(ctx, &name, current, pending, unprivileged));
        }

        List::from_tiles(list)
            .grid_template_columns("auto 1fr")
            .into()
    }

    fn card_menu(&self, ctx: &PveLxcResourcesPanelContext, data: &PvePendingConfiguration) -> Html {
        let props = ctx.props();

        if props.readonly {
            return html! {};
        }

        let unprivileged = is_unprivileged(data);

        let menu = Menu::new().with_item({
            MenuItem::new(tr!("Add Mount Point"))
                .icon_class("fa fa-hdd-o")
                .on_select(ctx.link().callback({
                    let property = lxc_mount_point_property(
                        None,
                        Some(props.node.clone()),
                        props.remote.clone(),
                        unprivileged,
                        true,
                    );
                    move |_| PendingPropertyViewMsg::AddProperty(property.clone(), None)
                }))
        });

        MenuButton::new("")
            .icon_class("fa fa-bars")
            .class("circle")
            .menu(menu)
            .into()
    }
}

impl PendingPropertyView for PveLxcResourcesPanel {
    type Message = Msg;
    type Properties = LxcResourcesPanel;
    const MOBILE: bool = true;

    fn create(_ctx: &PveLxcResourcesPanelContext) -> Self {
        Self {
            view_state: PendingPropertyViewState::default(),
        }
    }

    fn changed(&mut self, ctx: &PveLxcResourcesPanelContext, old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        if props.node != old_props.node
            || props.vmid != old_props.vmid
            || props.remote != old_props.remote
        {
            ctx.link().send_message(PendingPropertyViewMsg::Load);
        }
        true
    }

    fn update(&mut self, ctx: &PveLxcResourcesPanelContext, msg: Self::Message) -> bool {
        let props = ctx.props();

        match msg {
            Msg::ResizeDisk(name) => {
                let dialog = props.resize_disk_dialog(&name).on_done(
                    ctx.link()
                        .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                );
                self.dialog = Some(dialog.into());
            }
            Msg::ReassignDisk(name) => {
                let dialog = props.reassign_volume_dialog(&name).on_done(
                    ctx.link()
                        .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                );
                self.dialog = Some(dialog.into());
            }

            Msg::MoveDisk(name) => {
                let dialog = props.move_volume_dialog(&name).on_done(
                    ctx.link()
                        .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                );
                self.dialog = Some(dialog.into());
            }
        }
        true
    }

    fn view(&self, ctx: &PveLxcResourcesPanelContext) -> Html {
        let title = tr!("Resources");
        let min_height = 200;

        let card = match (&self.data, &self.error) {
            (None::<_>, None::<_>) => standard_card(title, (), ())
                .class(pwt::css::Display::Flex)
                .class(pwt::css::FlexDirection::Column)
                .min_height(min_height)
                .with_child(pwt::widget::Progress::new().class("pwt-delay-visibility"))
                .with_child(
                    Container::new()
                        .class(pwt::css::FlexFit)
                        .class("pwt-bg-color-neutral"),
                ),
            (None::<_>, Some(err)) => standard_card(title, (), ())
                .class(pwt::css::Display::Flex)
                .class(pwt::css::FlexDirection::Column)
                .min_height(min_height)
                .with_child(
                    pwt::widget::error_message(&err.to_string())
                        .padding(2)
                        .class(pwt::css::FlexFit)
                        .class("pwt-bg-color-neutral"),
                ),
            (Some(data), Some(err)) => {
                let card_menu = self.card_menu(ctx, data);
                standard_card(title, (), card_menu)
                    .with_child(
                        pwt::widget::error_message(&err.to_string())
                            .padding(2)
                            .border_bottom(true)
                            .class("pwt-bg-color-neutral"),
                    )
                    .with_child(self.view_list(ctx, data))
            }
            (Some(data), None::<_>) => {
                let card_menu = self.card_menu(ctx, data);
                standard_card(title, (), card_menu).with_child(self.view_list(ctx, data))
            }
        };
        card.with_optional_child(self.dialog.clone()).into()
    }

    fn editor_loader(props: &Self::Properties) -> Option<crate::ApiLoadCallback<Value>> {
        let url = guest_config_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);
        Some(typed_load::<LxcConfig>(url.clone()))
    }

    fn pending_loader(
        props: &Self::Properties,
    ) -> Option<crate::ApiLoadCallback<PvePendingConfiguration>> {
        let url = guest_pending_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);
        Some(pending_typed_load::<LxcConfig>(url.clone()))
    }

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>> {
        let url = guest_config_url(props.vmid, &props.node, &props.remote, PveGuestType::Lxc);
        Some(super::create_on_submit(
            url,
            props.on_start_command.clone(),
            false,
            0,
        ))
    }
}
