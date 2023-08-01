use yew::AttrValue;
use proxmox_schema::Schema;

pub trait SchemaValidation {
    fn schema(mut self, schema: &'static Schema) -> Self where Self: Sized {
        self.set_schema(schema);
        self
    }

    fn set_schema(&mut self, schema: &'static Schema);
}


impl SchemaValidation for pwt::widget::form::Field {
    fn set_schema(&mut self, schema: &'static Schema) {
        match schema {
            Schema::Integer(s) => {
                self.min = s.minimum.map(|v| v as f64);
                self.max = s.maximum.map(|v| v as f64);
                self.step = Some(1.0);
                self.input_type = AttrValue::Static("number");
            }
            Schema::Number(s) => {
                self.min = s.minimum;
                self.max = s.maximum;
                self.step = Some(1.0);
                self.input_type = AttrValue::Static("number");
            }
            _ => {}
        }
        self.set_validate(move |value: &String| {
            schema.parse_simple_value(value)?;
            Ok(())
        });
    }
}