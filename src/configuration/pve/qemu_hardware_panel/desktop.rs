use std::rc::Rc;

use proxmox_schema::property_string::PropertyString;
use serde_json::Value;

use yew::prelude::*;
use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::props::{ExtractPrimaryKey, SubmitCallback};
use pwt::state::{Selection, Store};
use pwt::widget::data_table::{
    DataTable, DataTableColumn, DataTableHeader, DataTableKeyboardEvent, DataTableMouseEvent,
};
use pwt::widget::menu::{Menu, MenuButton, MenuItem};
use pwt::widget::{Button, Column, Container, Fa, Row, Toolbar};

use pve_api_types::{
    PveQmIde, PveQmIdeMedia, QemuConfig, QemuConfigIdeArray, QemuConfigNetArray, QemuConfigSata,
    QemuConfigSataArray, QemuConfigScsi, QemuConfigScsiArray, QemuConfigUnusedArray,
    QemuConfigVirtioArray,
};

use crate::form::pve::{
    qemu_bios_property, qemu_cdrom_property, qemu_disk_property, qemu_display_property,
    qemu_efidisk_property, qemu_machine_property, qemu_memory_property, qemu_network_property,
    qemu_scsihw_property, qemu_sockets_cores_property, qemu_tpmstate_property,
    qemu_unused_disk_property, qemu_vmstate_property, typed_load,
};
use crate::pending_property_view::{
    pending_typed_load, render_pending_property_value, PendingPropertyView, PendingPropertyViewMsg,
    PendingPropertyViewState, PvePendingConfiguration, PvePendingPropertyView,
};
use crate::{EditableProperty, SafeConfirmDialog};

use super::{EditAction, QemuHardwarePanel};

pub enum Msg {
    ResizeDisk(String),
    ReassignDisk(String),
    MoveDisk(String),
}

#[derive(Clone, PartialEq)]
struct HardwareEntry {
    pub key: Key,
    pub property: EditableProperty,
    pub icon: Fa,
    pub header: Html,
    pub content: Html,
    pub has_changes: bool,
    pub is_disk: bool,
    edit_action: EditAction,
}

impl ExtractPrimaryKey for HardwareEntry {
    fn extract_key(&self) -> Key {
        Key::from(self.key.clone())
    }
}

pub struct PveQemuHardwarePanel {
    store: Store<HardwareEntry>,
    columns: Rc<Vec<DataTableHeader<HardwareEntry>>>,
    selection: Selection,
}

impl PveQemuHardwarePanel {
    fn toolbar(
        &self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &PendingPropertyViewState,
    ) -> Html {
        let link = ctx.link();

        let selected_key = self.selection.selected_key();
        let selected_record = selected_key
            .as_ref()
            .map(|key| self.store.read().lookup_record(&key).cloned())
            .flatten();
        let has_changes = selected_record
            .as_ref()
            .map(|record| record.has_changes)
            .unwrap_or(false);

        let property = selected_record.as_ref().map(|r| r.property.clone());

        let disable_revert = !(has_changes && selected_key.is_some());

        let (disable_remove, remove_label, remove_message) = match &selected_record {
            Some(record) => {
                let disable = record.property.required;
                let label = if record.is_disk {
                    tr!("Detach")
                } else {
                    tr!("Remove")
                };
                let message = if record.is_disk {
                    tr!("Detach disk")
                } else {
                    tr!("Delete Device")
                };
                (disable, label, message)
            }
            None::<_> => (true, tr!("Remove"), tr!("Remove")),
        };

        let disable_disk_actions = match &selected_record {
            Some(record) => !record.is_disk,
            None::<_> => true,
        };

        let toolbar = Toolbar::new()
            .class("pwt-overflow-hidden")
            .class("pwt-border-bottom")
            .with_child(self.add_hardware_menu(ctx, view_state))
            .with_child(
                Button::new(remove_label)
                    .disabled(disable_remove)
                    .on_activate({
                        let dialog = selected_key.clone().map(|name| {
                            SafeConfirmDialog::new(name.to_string())
                                .message(remove_message)
                                .on_done(
                                    link.callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                                )
                                .on_confirm(link.callback({
                                    let name = name.to_string();
                                    move |_| PendingPropertyViewMsg::Delete(name.clone())
                                }))
                                .into()
                        });
                        ctx.link()
                            .callback(move |_| PendingPropertyViewMsg::ShowDialog(dialog.clone()))
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
                                link.send_message(PendingPropertyViewMsg::EditProperty(
                                    property.clone(),
                                ));
                            }
                        }
                    }),
            )
            .with_child(
                self.disk_actions_menu(ctx, selected_key.clone())
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
                                link.send_message(PendingPropertyViewMsg::RevertProperty(
                                    property.clone(),
                                ));
                            }
                        }
                    }),
            );

        toolbar.into()
    }

    fn disk_actions_menu(
        &self,
        ctx: &PveQemuHardwarePanelContext,
        name: Option<Key>,
    ) -> MenuButton {
        let mut menu = Menu::new();

        if let Some(name) = name {
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Move Disk")).on_select(
                    ctx.link().callback(move |_| {
                        PendingPropertyViewMsg::Custom(Msg::MoveDisk(name.clone()))
                    }),
                )
            });
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Reassign Disk")).on_select(ctx.link().callback(move |_| {
                    PendingPropertyViewMsg::Custom(Msg::ReassignDisk(name.clone()))
                }))
            });
            menu.add_item({
                let name = name.to_string();
                MenuItem::new(tr!("Resize Disk")).on_select(ctx.link().callback(move |_| {
                    PendingPropertyViewMsg::Custom(Msg::ResizeDisk(name.clone()))
                }))
            });
        }

        MenuButton::new(tr!("Disk Actions"))
            .show_arrow(true)
            .menu(menu)
    }

    fn add_hardware_menu(
        &self,
        ctx: &PveQemuHardwarePanelContext,
        view_state: &PendingPropertyViewState,
    ) -> Html {
        let props = ctx.props();

        let PvePendingConfiguration {
            current: _,
            pending,
            keys: _,
        } = match &view_state.data {
            Some(data) => data,
            _ => &PvePendingConfiguration::new(),
        };

        let has_efidisk = pending.get("efidisk0").is_some();
        let has_tpmstate = pending.get("tpmstate0").is_some();

        let menu = Menu::new()
            .with_item({
                MenuItem::new(tr!("Add Hard Disk"))
                    .icon_class("fa fa-hdd-o")
                    .on_select(ctx.link().callback({
                        let property = qemu_disk_property(None, Some(props.node.clone()));
                        move |_| PendingPropertyViewMsg::AddProperty(property.clone())
                    }))
            })
            .with_item({
                MenuItem::new(tr!("Add CD/DVD drive"))
                    .icon_class("fa fa-cdrom")
                    .on_select(ctx.link().callback({
                        let property = qemu_cdrom_property(
                            None,
                            Some(props.node.clone()),
                            props.remote.clone(),
                            false,
                        );
                        move |_| PendingPropertyViewMsg::AddProperty(property.clone())
                    }))
            })
            .with_item({
                MenuItem::new(tr!("Add Network card"))
                    .icon_class("fa fa-exchange")
                    .on_select(ctx.link().callback({
                        let property = qemu_network_property(None, Some(props.node.clone()));
                        move |_| PendingPropertyViewMsg::AddProperty(property.clone())
                    }))
            })
            .with_item({
                MenuItem::new(tr!("EFI Disk"))
                    .icon_class("fa fa-hdd-o")
                    .disabled(has_efidisk)
                    .on_select(ctx.link().callback({
                        let property = qemu_efidisk_property(None, Some(props.node.clone()));
                        move |_| PendingPropertyViewMsg::AddProperty(property.clone())
                    }))
            })
            .with_item({
                MenuItem::new(tr!("TPM State"))
                    .icon_class("fa fa-hdd-o")
                    .disabled(has_tpmstate)
                    .on_select(ctx.link().callback({
                        let property = qemu_tpmstate_property(None, Some(props.node.clone()));
                        move |_| PendingPropertyViewMsg::AddProperty(property.clone())
                    }))
            });

        MenuButton::new(tr!("Add"))
            .show_arrow(true)
            .menu(menu)
            .into()
    }
}

type PveQemuHardwarePanelContext = Context<PvePendingPropertyView<PveQemuHardwarePanel>>;

impl PendingPropertyView for PveQemuHardwarePanel {
    type Message = Msg;
    type Properties = QemuHardwarePanel;
    const MOBILE: bool = false;

    fn create(ctx: &PveQemuHardwarePanelContext) -> Self {
        let selection = Selection::new().on_select({
            let link = ctx.link().clone();
            move |selection: Selection| {
                let selected_key = selection.selected_key();
                link.send_message(PendingPropertyViewMsg::Select(selected_key.clone()));
            }
        });

        Self {
            store: Store::new(),
            columns: columns(),
            selection,
        }
    }

    fn update_data(
        &mut self,
        ctx: &Context<PvePendingPropertyView<Self>>,
        view_state: &mut PendingPropertyViewState,
    ) where
        Self: 'static + Sized,
    {
        let props = ctx.props();
        let mut list: Vec<HardwareEntry> = Vec::new();

        let PvePendingConfiguration {
            current,
            pending,
            keys,
        } = match &view_state.data {
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

            HardwareEntry {
                key: name.into(),
                header,
                content: content.into(),
                icon,
                has_changes,
                is_disk: false,
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

        let push_disk_property = |list: &mut Vec<_>, name: &str, media| {
            let (property, icon, is_disk) = if media == PveQmIdeMedia::Cdrom {
                (
                    qemu_cdrom_property(
                        Some(name.to_string()),
                        Some(props.node.clone()),
                        props.remote.clone(),
                        false,
                    ),
                    Fa::new("cdrom"),
                    false,
                )
            } else {
                (
                    qemu_disk_property(Some(name.to_string()), Some(props.node.clone())),
                    Fa::new("hdd-o"),
                    true,
                )
            };
            let mut entry = create_entry(&name, property, icon, EditAction::Edit);
            entry.is_disk = is_disk;
            list.push(entry);
        };

        let push_network_property = |list: &mut Vec<_>, name: &str| {
            let icon = Fa::new("exchange");
            // fixme: add remote
            let property = qemu_network_property(Some(name.to_string()), Some(props.node.clone()));
            let entry = create_entry(&name, property, icon, EditAction::Edit);
            list.push(entry);
        };

        let push_unused_disk_property = |list: &mut Vec<_>, name: &str| {
            let icon = Fa::new("hdd-o");
            let property = qemu_unused_disk_property(&name, Some(props.node.clone()));
            let entry = create_entry(&name, property, icon, EditAction::Add);
            list.push(entry);
        };

        push_property(
            &mut list,
            qemu_memory_property(),
            Fa::new("memory"),
            EditAction::Edit,
        );
        push_property(
            &mut list,
            qemu_sockets_cores_property(false),
            Fa::new("cpu"),
            EditAction::Edit,
        );
        push_property(
            &mut list,
            qemu_bios_property(),
            Fa::new("microchip"),
            EditAction::Edit,
        );
        push_property(
            &mut list,
            qemu_display_property(),
            Fa::new("desktop"),
            EditAction::Edit,
        );
        push_property(
            &mut list,
            qemu_machine_property(),
            Fa::new("cogs"),
            EditAction::Edit,
        );
        push_property(
            &mut list,
            qemu_scsihw_property(),
            Fa::new("database"),
            EditAction::Edit,
        );

        // fixme: this should be removable - add menu with delete
        push_property(
            &mut list,
            qemu_vmstate_property(),
            Fa::new("download"),
            EditAction::Edit,
        );

        for n in 0..QemuConfigIdeArray::MAX {
            let name = format!("ide{n}");
            if !keys.contains(&name) {
                continue;
            }
            let media = match serde_json::from_value::<Option<PropertyString<PveQmIde>>>(
                pending[&name].clone(),
            ) {
                Ok(Some(ide)) => ide.media.unwrap_or(PveQmIdeMedia::Disk),
                Ok(None::<_>) => PveQmIdeMedia::Disk,
                Err(err) => {
                    log::error!("unable to parse drive '{name}' media: {err}");
                    continue;
                }
            };
            push_disk_property(&mut list, &name, media);
        }

        for n in 0..QemuConfigSataArray::MAX {
            let name = format!("sata{n}");
            if !keys.contains(&name) {
                continue;
            }
            let media = match serde_json::from_value::<Option<PropertyString<QemuConfigSata>>>(
                pending[&name].clone(),
            ) {
                Ok(Some(ide)) => ide.media.unwrap_or(PveQmIdeMedia::Disk),
                Ok(None::<_>) => PveQmIdeMedia::Disk,
                Err(err) => {
                    log::error!("unable to parse drive '{name}' media: {err}");
                    continue;
                }
            };
            push_disk_property(&mut list, &name, media);
        }

        for n in 0..QemuConfigScsiArray::MAX {
            let name = format!("scsi{n}");
            if !keys.contains(&name) {
                continue;
            }
            let media = match serde_json::from_value::<Option<PropertyString<QemuConfigScsi>>>(
                pending[&name].clone(),
            ) {
                Ok(Some(scsi)) => scsi.media.unwrap_or(PveQmIdeMedia::Disk),
                Ok(None::<_>) => PveQmIdeMedia::Disk,
                Err(err) => {
                    log::error!("unable to parse drive '{name}' media: {err}");
                    continue;
                }
            };
            push_disk_property(&mut list, &name, media);
        }

        for n in 0..QemuConfigVirtioArray::MAX {
            let name = format!("virtio{n}");
            if !keys.contains(&name) {
                continue;
            }
            push_disk_property(&mut list, &name, PveQmIdeMedia::Disk);
        }

        for n in 0..QemuConfigNetArray::MAX {
            let name = format!("net{n}");
            if !keys.contains(&name) {
                continue;
            }
            push_network_property(&mut list, &name);
        }

        for n in 0..QemuConfigUnusedArray::MAX {
            let name = format!("unused{n}");
            if !keys.contains(&name) {
                continue;
            }
            push_unused_disk_property(&mut list, &name);
        }
        self.store.set_data(list);
    }

    fn changed(
        &mut self,
        ctx: &PveQemuHardwarePanelContext,
        _view_state: &mut PendingPropertyViewState,
        old_props: &Self::Properties,
    ) -> bool {
        let props = ctx.props();

        if props.node != old_props.node
            || props.vmid != old_props.vmid
            || props.remote != old_props.remote
        {
            ctx.link().send_message(PendingPropertyViewMsg::Load);
        }
        true
    }

    fn update(
        &mut self,
        ctx: &PveQemuHardwarePanelContext,
        view_state: &mut PendingPropertyViewState,
        msg: Self::Message,
    ) -> bool {
        let props = ctx.props();

        match msg {
            Msg::ResizeDisk(name) => {
                let dialog = props.resize_disk_dialog(&name).on_done(
                    ctx.link()
                        .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                );
                view_state.dialog = Some(dialog.into());
            }
            Msg::ReassignDisk(name) => {
                let dialog = props.reassign_disk_dialog(&name).on_done(
                    ctx.link()
                        .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                );
                view_state.dialog = Some(dialog.into());
            }
            Msg::MoveDisk(name) => {
                let dialog = props.move_disk_dialog(&name).on_done(
                    ctx.link()
                        .callback(|_| PendingPropertyViewMsg::ShowDialog(None)),
                );
                view_state.dialog = Some(dialog.into());
            }
        }
        true
    }

    fn view(
        &self,
        ctx: &PveQemuHardwarePanelContext,
        view_state: &PendingPropertyViewState,
    ) -> Html {
        let table = DataTable::new(self.columns.clone(), self.store.clone())
            .class(pwt::css::FlexFit)
            .show_header(false)
            .virtual_scroll(false)
            .selection(self.selection.clone())
            .on_row_dblclick({
                let link = ctx.link().clone();
                let store = self.store.clone();
                move |event: &mut DataTableMouseEvent| {
                    let record = store.read().lookup_record(&event.record_key).cloned();
                    if let Some(record) = record {
                        match record.edit_action {
                            EditAction::None => {}
                            EditAction::Add => link
                                .send_message(PendingPropertyViewMsg::AddProperty(record.property)),
                            EditAction::Edit => link.send_message(
                                PendingPropertyViewMsg::EditProperty(record.property),
                            ),
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
                                EditAction::None => {}
                                EditAction::Add => link.send_message(
                                    PendingPropertyViewMsg::AddProperty(record.property),
                                ),
                                EditAction::Edit => link.send_message(
                                    PendingPropertyViewMsg::EditProperty(record.property),
                                ),
                            }
                        }
                    }
                }
            })
            .into();

        let loading = view_state.loading();
        let toolbar = self.toolbar(ctx, view_state);
        let class = classes!(pwt::css::FlexFit);
        let dialog = view_state.dialog.clone();
        let error = view_state.error.clone();

        crate::property_view::render_loadable_panel(
            class,
            table,
            Some(toolbar),
            dialog,
            loading,
            error,
        )
    }

    fn editor_loader(props: &Self::Properties) -> Option<crate::ApiLoadCallback<Value>> {
        let url = props.editor_url();
        Some(typed_load::<QemuConfig>(url.clone()))
    }

    fn pending_loader(
        props: &Self::Properties,
    ) -> Option<crate::ApiLoadCallback<PvePendingConfiguration>> {
        let pending_url = props.pending_url();
        Some(pending_typed_load::<QemuConfig>(pending_url.clone()))
    }

    fn on_submit(props: &Self::Properties) -> Option<SubmitCallback<Value>> {
        Some(super::create_on_submit(
            props.editor_url(),
            props.on_start_command.clone(),
        ))
    }
}

fn columns() -> Rc<Vec<DataTableHeader<HardwareEntry>>> {
    Rc::new(vec![
        DataTableColumn::new(tr!("Key"))
            .show_menu(false)
            .render(|record: &HardwareEntry| {
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
            .render(|record: &HardwareEntry| record.content.clone())
            .into(),
    ])
}
