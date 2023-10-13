use std::cell::RefCell;
use std::collections::HashMap;

use proxmox_schema::Schema;
use pwt::widget::form::{ValidateFn, InputType};

pub trait SchemaValidation {
    fn schema(mut self, schema: &'static Schema) -> Self
    where
        Self: Sized,
    {
        self.set_schema(schema);
        self
    }

    fn set_schema(&mut self, schema: &'static Schema);
}

// We use one ValidateFn per Schema (to avoid/minimize property changes).
thread_local! {
    static VALIDATION_FN_MAP: RefCell<HashMap<usize, ValidateFn<String>>> = RefCell::new(HashMap::new());
}

impl SchemaValidation for pwt::widget::form::Field {
    fn set_schema(&mut self, schema: &'static Schema) {
        // Note: All our schemas are static, so we can use the pointer to
        // identify them uniquely. Not ideal, but good enough.
        let schema_id = schema as *const Schema as usize;

        match schema {
            Schema::Integer(s) => {
                self.min = s.minimum.map(|v| v as f64);
                self.max = s.maximum.map(|v| v as f64);
                self.step = Some(1.0);
                self.input_type = InputType::Number;
            }
            Schema::Number(s) => {
                self.min = s.minimum;
                self.max = s.maximum;
                self.step = Some(1.0);
                self.input_type = InputType::Number;
            }
            _ => {}
        }

        let validate = VALIDATION_FN_MAP.with(|cell| {
            let mut map = cell.borrow_mut();
            if let Some(validate) = map.get(&schema_id) {
                validate.clone()
            } else {
                let validate = ValidateFn::new(|value: &String| {
                    schema.parse_simple_value(value)?;
                    Ok(())
                });
                map.insert(schema_id, validate.clone());
                validate
            }
        });

        self.set_validate(validate);
    }
}
