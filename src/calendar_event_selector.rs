use std::rc::Rc;

use yew::prelude::*;
use yew::virtual_dom::Key;
use yew::html::IntoPropValue;

use pwt::props::RenderFn;
use pwt::state::Store;
use pwt::widget::data_table::{DataTable, DataTableColumn, DataTableHeader};
use pwt::widget::GridPicker;

use pwt::widget::form::{Selector, SelectorRenderArgs, ValidateFn};

type Record = (&'static str, &'static str);

static EXAMPLES: &[Record] = &[
    ("*:0/30", "Every 30 minutes"),
    ("hourly", "Every hour"),
    ("0/2:00", "Every two hours"),
    ("2,22:30", "Every day 02:30, 22:30"),
    ("21:00", "Every day 21:00"),
    ("daily", "Every day 00:00"),
    ("mon..fri 00:00", "Monday to Friday 00:00"),
    ("mon..fri *:00", "Monday to Friday, hourly"),
    ("sat 18:15", "Every Saturday 18:15"),
    ("monthly", "Every first day of the Month 00:00"),
    ("sat *-1..7 02:00", "Every first Saturday of the month 02:00"),
    ("yearly", "First day of the year 00:00"),
];

thread_local!{
    static COLUMNS: Rc<Vec<DataTableHeader<Record>>> = Rc::new(vec![
        DataTableColumn::new("Value")
            .width("100px")
            .show_menu(false)
            .render(|item: &Record| html!{item.0.clone()})
            .into(),
        DataTableColumn::new("Comment")
            .width("300px")
            .show_menu(false)
            .render(|item: &Record| html!{item.1.clone()})
            .into(),
    ]);
}


use pwt_macros::{builder, widget};
use pwt::props::{FieldBuilder, WidgetBuilder};

#[widget(comp=ProxmoxCalendarEventSelector, @input)]
#[derive(Clone, Properties, PartialEq)]
#[builder]
pub struct CalendarEventSelector {
    /// The default value.
    #[builder(IntoPropValue, into_prop_value)]
    #[prop_or_default]
    pub default: Option<AttrValue>,
}

impl CalendarEventSelector {
    /// Create a new instance.
    pub fn new() -> Self {
        yew::props!(Self {})
    }
}

pub struct ProxmoxCalendarEventSelector {
    store: Store<Record>,
    validate: ValidateFn<(String, Store<Record>)>,
    picker: RenderFn<SelectorRenderArgs<Store<Record>>>,
}

impl Component for ProxmoxCalendarEventSelector {
    type Message = ();
    type Properties =  CalendarEventSelector;

    fn create(_ctx: &Context<Self>) -> Self {
        let store = Store::with_extract_key(|item: &Record| {
            Key::from(item.0.clone())
        });
        store.set_data(Vec::from(EXAMPLES));

        let validate = ValidateFn::new(|(value, _store): &(String, Store<Record>)| {
            proxmox_time::verify_calendar_event(value)
        });

        let picker = RenderFn::new(|args: &SelectorRenderArgs<Store<Record>>| {

            let table = DataTable::new(COLUMNS.with(Rc::clone), args.store.clone())
                .class("pwt-fit");

            GridPicker::new(table)
                .show_filter(false)
                .selection(args.selection.clone())
                .on_select(args.on_select.clone())
                .into()
        });

        Self { store, validate, picker }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();

        Selector::new(self.store.clone(), self.picker.clone())
            .with_std_props(&props.std_props)
            .with_input_props(&props.input_props)
            .default(&props.default)
            .editable(true)
            .validate(self.validate.clone())
            .into()
    }
}
