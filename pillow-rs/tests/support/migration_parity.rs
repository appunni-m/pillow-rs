use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub case_id: String,
    pub operation: String,
    pub inputs: Inputs,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub assets: Value,
    pub params: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublicError {
    pub class: String,
    pub kind: String,
    pub message: String,
    pub stage: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultEnvelope {
    pub case_id: String,
    pub status: Status,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub error: Option<PublicError>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum OutputShape {
    Object,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diff {
    pub field: &'static str,
    pub source: String,
    pub target: String,
}

impl ResultEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        match self.status {
            Status::Ok if self.value.is_some() && self.error.is_none() => Ok(()),
            Status::Error if self.value.is_none() && self.error.is_some() => Ok(()),
            Status::Ok => Err(format!(
                "{}: ok Result must contain value and no error",
                self.case_id
            )),
            Status::Error => Err(format!(
                "{}: error Result must contain error and no value",
                self.case_id
            )),
        }
    }
}

pub fn compare_results(
    source_system: &str,
    target_system: &str,
    source: &ResultEnvelope,
    target: &ResultEnvelope,
    output_shape: OutputShape,
) -> Result<Vec<Diff>, String> {
    if source_system == target_system {
        return Err("source and target identities must differ".into());
    }
    source.validate()?;
    target.validate()?;

    let mut diffs = Vec::new();
    if source.case_id != target.case_id {
        diffs.push(Diff {
            field: "case_id",
            source: source.case_id.clone(),
            target: target.case_id.clone(),
        });
        return Ok(diffs);
    }
    if source.status != target.status {
        diffs.push(Diff {
            field: "status",
            source: format!("{:?}", source.status),
            target: format!("{:?}", target.status),
        });
        return Ok(diffs);
    }

    match source.status {
        Status::Ok => {
            let source_value = source
                .value
                .as_ref()
                .expect("validated source ok Result has a value");
            let target_value = target
                .value
                .as_ref()
                .expect("validated target ok Result has a value");
            match output_shape {
                OutputShape::Object => {
                    if !source_value.is_object() || !target_value.is_object() {
                        diffs.push(Diff {
                            field: "value.shape",
                            source: json_shape(source_value).into(),
                            target: json_shape(target_value).into(),
                        });
                    } else if source_value != target_value {
                        diffs.push(Diff {
                            field: "value",
                            source: source_value.to_string(),
                            target: target_value.to_string(),
                        });
                    }
                }
            }
        }
        Status::Error => {
            let source_error = source
                .error
                .as_ref()
                .expect("validated source error Result has an error");
            let target_error = target
                .error
                .as_ref()
                .expect("validated target error Result has an error");
            if source_error != target_error {
                diffs.push(Diff {
                    field: "error",
                    source: format!("{source_error:?}"),
                    target: format!("{target_error:?}"),
                });
            }
        }
    }
    Ok(diffs)
}

fn json_shape(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
