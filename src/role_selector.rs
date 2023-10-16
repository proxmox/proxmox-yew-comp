use anyhow::format_err;
use std::rc::Rc;

use yew::virtual_dom::Key;

use pwt::prelude::*;
use pwt::props::RenderFn;
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};
use pwt::widget::GridPicker;

use crate::common_api_types::RoleInfo;

thread_local! {
    static COLUMNS: Rc<Vec<DataTableHeader<RoleInfo>>> = Rc::new(vec![
        DataTableColumn::new(tr!("Role"))
            .width("200px")
            .show_menu(false)
            .render(|record: &RoleInfo| {
                html!{record.roleid.clone()}
            })
            .sorter(|a: &RoleInfo, b: &RoleInfo| {
                a.roleid.cmp(&b.roleid)
            })
            .sort_order(true)
            .into(),
        DataTableColumn::new(tr!("Privileges"))
            .width("400px")
            .show_menu(false)
            .render(|record: &RoleInfo| {
                let text = record.privs.join(" ");
                html!{<span class="pwt-white-space-normal">{text}</span>}
            })
            .into(),
    ]);
}

use pwt::props::{FieldBuilder, WidgetBuilder};
use pwt_macros::widget;

#[widget(comp=ProxmoxRoleSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
pub struct RoleSelector {}

impl RoleSelector {
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxRoleSelector {
    store: Store<RoleInfo>,
    validate: ValidateFn<(String, Store<RoleInfo>)>,
    picker: RenderFn<SelectorRenderArgs<Store<RoleInfo>>>,
}

impl Component for ProxmoxRoleSelector {
    type Message = ();
    type Properties = RoleSelector;

    fn create(ctx: &Context<Self>) -> Self {
        let store = Store::with_extract_key(|item: &RoleInfo| Key::from(item.roleid.clone()))
            .on_change(ctx.link().callback(|_| ())); // trigger redraw

        let validate = ValidateFn::new(|(role, store): &(String, Store<RoleInfo>)| {
            store
                .read()
                .data()
                .iter()
                .find(|item| &item.roleid == role)
                .map(drop)
                .ok_or_else(|| format_err!("no such Role"))
        });

        let picker = RenderFn::new(|args: &SelectorRenderArgs<Store<RoleInfo>>| {
            let table =
                DataTable::new(COLUMNS.with(Rc::clone), args.store.clone()).class("pwt-fit");

            GridPicker::new(table)
                .show_filter(false)
                .selection(args.selection.clone())
                .on_select(args.on_select.clone())
                .into()
        });

        Self {
            store,
            validate,
            picker,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Selector::new(self.store.clone(), self.picker.clone())
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .required(true)
            .default("NoAccess")
            .loader("/access/roles")
            .validate(self.validate.clone())
            .into()
    }
}
