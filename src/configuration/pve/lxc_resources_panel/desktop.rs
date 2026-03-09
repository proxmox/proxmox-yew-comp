use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use pwt::state::{Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableHeader, DataTableKeyboardEvent, DataTableMouseEvent,
};
use serde_json::Value;

use pwt::prelude::*;
use pwt::widget::data_table::DataTableColumn;
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::{Button, Column, Container, Fa, Row, Toolbar};

use pwt::props::{ExtractPrimaryKey, SubmitCallback};

use pve_api_types::{LxcConfig, LxcConfigMpArray, LxcConfigUnusedArray};
use yew::virtual_dom::Key;

use crate::configuration::pve::guest::{
    confirm_delete_volume, confirm_detach_entry, confirm_remove_entry,
};
use crate::configuration::pve::lxc_resources_panel::is_unprivileged;
use crate::configuration::{guest_config_url, guest_pending_url};
use crate::form::pve::{
    lxc_cores_property, lxc_memory_property, lxc_mount_point_property, lxc_rootfs_property,
    lxc_swap_property, lxc_unused_volume_property, PveGuestType,
};
use crate::form::typed_load;
use crate::pending_property_view::{
    pending_typed_load, render_pending_property_value, PendingPropertyView,
    PendingPropertyViewScopeExt, PendingPropertyViewState, PvePendingConfiguration,
    PvePendingPropertyView,
};
use crate::EditableProperty;

use super::{EditAction, LxcResourcesPanel, Msg};

#[derive(Copy, Clone, PartialEq)]
enum EntryType {
    Other,
    MountPoint,
    Rootfs,
    Unused,
}

#[derive(Clone, PartialEq)]
struct ResourceEntry {
    pub key: Key,
    pub property: EditableProperty,
    pub icon: Fa,
    pub header: Html,
    pub content: Html,
    pub has_changes: bool,
    pub entry_type: EntryType,
    edit_action: EditAction,
}

impl ExtractPrimaryKey for ResourceEntry {
    fn extract_key(&self) -> Key {
        self.key.clone()
    }
}

pub struct PveLxcResourcesPanel {
    view_state: PendingPropertyViewState,

    store: Store<ResourceEntry>,
    columns: Rc<Vec<DataTableHeader<ResourceEntry>>>,
    selection: Selection,
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
    fn lookup_property_value(&self, name: &str) -> Option<Value> {
        let PvePendingConfiguration {
            current: _,
            pending,
            keys: _,
        } = match &self.data {
            Some(data) => data,
            _ => &PvePendingConfiguration::new(),
        };
        pending.get(name).cloned()
    }

    fn toolbar(&self, ctx: &PveLxcResourcesPanelContext) -> Html {
        let link = ctx.link();

        let selected_key = self.selection.selected_key();
        let selected_record = selected_key
            .as_ref()
            .and_then(|key| self.store.read().lookup_record(key).cloned());
        let has_changes = selected_record
            .as_ref()
            .map(|record| record.has_changes)
            .unwrap_or(false);

        let property = selected_record.as_ref().map(|r| r.property.clone());

        let disable_revert = !(has_changes && selected_key.is_some());

        let (disable_remove, remove_label) = {
            match &selected_record {
                Some(record) => {
                    let disable = record.property.required;
                    let label = match record.entry_type {
                        EntryType::MountPoint => tr!("Detach"),
                        _ => tr!("Remove"),
                    };
                    (disable, label)
                }
                None::<_> => (true, tr!("Remove")),
            }
        };

        let entry_type = match &selected_record {
            Some(record) => record.entry_type,
            _ => EntryType::Other,
        };

        let disable_disk_actions = match entry_type {
            EntryType::Other => true,
            EntryType::MountPoint => false,
            EntryType::Rootfs => false,
            EntryType::Unused => false,
        };

        let on_done = {
            let link = ctx.link().clone();
            move |_| link.send_show_dialog(None)
        };

        let toolbar = Toolbar::new()
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(self.add_resources_menu(ctx))
            .with_child(
                Button::new(remove_label.clone())
                    .disabled(disable_remove)
                    .on_activate({
                        let dialog = selected_key.clone().map(move |name| {
                            let on_confirm = {
                                let name = name.to_string();
                                let link = link.clone();
                                Callback::from(move |_| link.send_delete(&name, None))
                            };

                            match entry_type {
                                EntryType::Unused => {
                                    let volume = match self.lookup_property_value(&name) {
                                        Some(Value::String(volume)) => volume.clone(),
                                        _ => name.to_string(),
                                    };
                                    confirm_delete_volume(&*name, &volume, false)
                                        .on_close(on_done)
                                        .on_confirm({
                                            let on_confirm = on_confirm.clone();
                                            move |_| on_confirm.emit(())
                                        })
                                        .into()
                                }
                                EntryType::MountPoint => confirm_detach_entry(&name, false)
                                    .on_close(on_done)
                                    .on_confirm(on_confirm)
                                    .into(),
                                _ => confirm_remove_entry(&name, false)
                                    .on_close(on_done)
                                    .on_confirm(on_confirm)
                                    .into(),
                            }
                        });

                        let link = link.clone();
                        move |_| link.send_show_dialog(dialog.clone())
                    }),
            )
            .with_child(
                Button::new(tr!("Edit"))
                    .disabled(selected_key.is_none())
                    .onclick({
                        let link = link.clone();
                        let property = property.clone();
                        move |_| {
                            if let Some(property) = &property {
                                link.send_edit_property(property.clone(), None);
                            }
                        }
                    }),
            )
            .with_child(
                self.disk_actions_menu(ctx, selected_key.clone(), entry_type)
                    .disabled(disable_disk_actions),
            )
            .with_child(
                Button::new(tr!("Revert"))
                    .disabled(disable_revert)
                    .onclick({
                        let link = link.clone();
                        let property = property.clone();
                        move |_| {
                            if let Some(property) = &property {
                                link.send_revert_property(property.clone());
                            }
                        }
                    }),
            );

        toolbar.into()
    }

    fn disk_actions_menu(
        &self,
        ctx: &PveLxcResourcesPanelContext,
        name: Option<Key>,
        entry_type: EntryType,
    ) -> MenuButton {
        let mut menu = Menu::new();

        let mut enable_move = false;
        let mut enable_reassign = false;
        let mut enable_resize = false;

        match entry_type {
            EntryType::Rootfs => {
                enable_move = true;
                enable_resize = true;
            }
            EntryType::MountPoint => {
                enable_move = true;
                enable_reassign = true;
                enable_resize = true;
            }
            EntryType::Unused => {
                enable_reassign = true;
            }
            EntryType::Other => { /* do nothing  */ }
        }

        if let Some(name) = name {
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Move Storage"))
                    .icon_class("fa fa-database")
                    .disabled(!enable_move)
                    .on_select(
                        ctx.link()
                            .custom_callback(move |_| Msg::MoveDisk(name.clone())),
                    )
            });
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Reassign Owner"))
                    .icon_class("fa fa-desktop")
                    .disabled(!enable_reassign)
                    .on_select(
                        ctx.link()
                            .custom_callback(move |_| Msg::ReassignDisk(name.clone())),
                    )
            });
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Resize"))
                    .icon_class("fa fa-plus")
                    .disabled(!enable_resize)
                    .on_select(
                        ctx.link()
                            .custom_callback(move |_| Msg::ResizeDisk(name.clone())),
                    )
            });
        }

        MenuButton::new(tr!("Disk Action"))
            .show_arrow(true)
            .menu(menu)
    }

    fn add_resources_menu(&self, ctx: &PveLxcResourcesPanelContext) -> Html {
        let props = ctx.props();

        let unprivileged = match &self.data {
            Some(data) => is_unprivileged(data),
            None => false,
        };

        let menu = Menu::new().with_item({
            MenuItem::new(tr!("Mount Point"))
                .icon_class("fa fa-hdd-o")
                .on_select({
                    let link = ctx.link().clone();
                    let property = lxc_mount_point_property(
                        None,
                        Some(props.node.clone()),
                        props.remote.clone(),
                        unprivileged,
                        false,
                    );
                    move |_| link.send_add_property(property.clone(), None)
                })
        });

        MenuButton::new(tr!("Add"))
            .show_arrow(true)
            .menu(menu)
            .into()
    }
}

impl PendingPropertyView for PveLxcResourcesPanel {
    type Message = Msg;
    type Properties = LxcResourcesPanel;
    const MOBILE: bool = false;

    fn create(ctx: &PveLxcResourcesPanelContext) -> Self {
        let selection = Selection::new().on_select({
            let link = ctx.link().clone();
            move |_| link.send_redraw()
        });

        Self {
            view_state: PendingPropertyViewState::default(),
            store: Store::new(),
            columns: columns(),
            selection,
        }
    }

    fn changed(&mut self, ctx: &PveLxcResourcesPanelContext, old_props: &Self::Properties) -> bool {
        let props = ctx.props();

        if props.node != old_props.node
            || props.vmid != old_props.vmid
            || props.remote != old_props.remote
        {
            ctx.link().send_reload();
        }
        true
    }

    fn update(&mut self, ctx: &PveLxcResourcesPanelContext, msg: Self::Message) -> bool {
        let props = ctx.props();
        let on_done = Callback::from({
            let link = ctx.link().clone();
            move |_| link.send_show_dialog(None)
        });
        match msg {
            Msg::ResizeDisk(name) => {
                let dialog = props.resize_disk_dialog(&name).on_done(on_done.clone());
                self.dialog = Some(dialog.into());
            }
            Msg::ReassignDisk(name) => {
                let dialog = props.reassign_volume_dialog(&name).on_done(on_done.clone());
                self.dialog = Some(dialog.into());
            }
            Msg::MoveDisk(name) => {
                let dialog = props.move_volume_dialog(&name).on_done(on_done.clone());
                self.dialog = Some(dialog.into());
            }
        }
        true
    }

    fn update_data(&mut self, ctx: &Context<PvePendingPropertyView<Self>>) {
        let props = ctx.props();
        let mut list: Vec<ResourceEntry> = Vec::new();

        let unprivileged = match &self.data {
            Some(data) => is_unprivileged(data),
            None => true,
        };

        let PvePendingConfiguration {
            current,
            pending,
            keys,
        } = match &self.data {
            Some(data) => data,
            _ => &PvePendingConfiguration::new(),
        };

        let create_entry = |name: &str, property: EditableProperty, icon, edit_action| {
            let header = property.title.clone().into();
            let (value, new_value) = render_pending_property_value(current, pending, &property);

            let mut content = Column::new().with_child(Container::new().with_child(value.clone()));

            let mut has_changes = false;

            if let Some(new_value) = new_value {
                has_changes = true;
                content.add_child(
                    Container::new()
                        .class("pwt-color-warning")
                        .with_child(new_value),
                );
            }

            ResourceEntry {
                key: name.into(),
                header,
                content: content.into(),
                icon,
                has_changes,
                entry_type: EntryType::Other,
                property,
                edit_action,
            }
        };

        let push_property = |list: &mut Vec<_>, property: EditableProperty, icon, edit_action| {
            let name = match property.get_name() {
                Some(name) => name.to_string(),
                None::<_> => return,
            };

            if property.required || keys.contains(&name) {
                list.push(create_entry(&name, property, icon, edit_action));
            }
        };

        let push_disk_property = |list: &mut Vec<_>, name: &str| {
            let property = lxc_mount_point_property(
                Some(name.to_string()),
                Some(props.node.clone()),
                props.remote.clone(),
                unprivileged,
                false,
            );
            let icon = Fa::new("hdd-o");

            let mut entry = create_entry(name, property, icon, EditAction::Edit);
            entry.entry_type = EntryType::MountPoint;
            list.push(entry);
        };

        let push_unused_disk_property = |list: &mut Vec<_>, name: &str| {
            let icon = Fa::new("hdd-o");
            let property = lxc_unused_volume_property(
                name.to_string(),
                Some(props.node.clone()),
                props.remote.clone(),
                unprivileged,
                false,
            );
            let mut entry = create_entry(name, property, icon, EditAction::Add);
            entry.entry_type = EntryType::Unused;
            list.push(entry);
        };

        push_property(
            &mut list,
            lxc_memory_property(false),
            Fa::new("memory"),
            EditAction::Edit,
        );

        push_property(
            &mut list,
            lxc_swap_property(false),
            Fa::new("retweet"),
            EditAction::Edit,
        );

        push_property(
            &mut list,
            lxc_cores_property(false),
            Fa::new("cpu"),
            EditAction::Edit,
        );

        {
            let property = lxc_rootfs_property(
                Some(props.node.clone()),
                props.remote.clone(),
                unprivileged,
                false,
            );
            let icon = Fa::new("hdd-o");
            let mut entry = create_entry("rootfs", property, icon, EditAction::Edit);
            entry.entry_type = EntryType::Rootfs;
            list.push(entry);
        }

        for n in 0..LxcConfigMpArray::MAX {
            let name = format!("mp{n}");
            if !keys.contains(&name) {
                continue;
            }
            push_disk_property(&mut list, &name);
        }

        for n in 0..LxcConfigUnusedArray::MAX {
            let name = format!("unused{n}");
            if !keys.contains(&name) {
                continue;
            }
            push_unused_disk_property(&mut list, &name);
        }

        self.store.set_data(list);
    }

    fn view(&self, ctx: &PveLxcResourcesPanelContext) -> Html {
        let props = ctx.props();

        let mut table = DataTable::new(self.columns.clone(), self.store.clone())
            .class(pwt::css::FlexFit)
            .show_header(false)
            .virtual_scroll(false)
            .selection(self.selection.clone());

        if !props.readonly {
            table = table
                .on_row_dblclick({
                    let link = ctx.link().clone();
                    let store = self.store.clone();
                    move |event: &mut DataTableMouseEvent| {
                        let record = store.read().lookup_record(&event.record_key).cloned();
                        if let Some(record) = record {
                            match record.edit_action {
                                //EditAction::None => {}
                                EditAction::Add => link.send_add_property(record.property, None),
                                EditAction::Edit => link.send_edit_property(record.property, None),
                            }
                        }
                    }
                })
                .on_row_keydown({
                    let link = ctx.link().clone();
                    let store = self.store.clone();
                    move |event: &mut DataTableKeyboardEvent| {
                        if event.key() == " " {
                            let record = store.read().lookup_record(&event.record_key).cloned();
                            if let Some(record) = record {
                                match record.edit_action {
                                    //EditAction::None => {}
                                    EditAction::Add => {
                                        link.send_add_property(record.property, None)
                                    }
                                    EditAction::Edit => {
                                        link.send_edit_property(record.property, None)
                                    }
                                }
                            }
                        }
                    }
                });
        }

        let table = table.into();
        let loading = self.loading();
        let toolbar = (!props.readonly).then(|| self.toolbar(ctx));
        let class = classes!(pwt::css::FlexFit);
        let dialog = self.dialog.clone();
        let error = self.error.clone();

        crate::property_view::render_loadable_panel(class, table, toolbar, dialog, loading, error)
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

fn columns() -> Rc<Vec<DataTableHeader<ResourceEntry>>> {
    Rc::new(vec![
        DataTableColumn::new(tr!("Key"))
            .show_menu(false)
            .render(|record: &ResourceEntry| {
                Row::new()
                    .gap(2)
                    .class(pwt::css::AlignItems::Center)
                    .with_child(record.icon.clone().class("fa-fw"))
                    .with_child(record.header.clone())
                    .into()
            })
            .into(),
        DataTableColumn::new(tr!("Value"))
            .show_menu(false)
            .render(|record: &ResourceEntry| record.content.clone())
            .into(),
    ])
}
