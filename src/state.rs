/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{collections::BTreeMap, convert::Infallible, fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use uuid::Uuid;

use crate::discovery::ServiceMeta;

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub(crate) struct TraceId(String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub(crate) struct SpanId(String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub(crate) struct ServiceNamespace(pub(crate) String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub(crate) struct ServiceName(String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub(crate) struct ServiceInstanceId(pub(crate) String);

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub(crate) struct OperationName(String);

#[derive(SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub(crate) struct ServiceKey {
    pub(crate) namespace: Option<ServiceNamespace>,
    pub(crate) name: ServiceName,
    pub(crate) instance_id: Option<ServiceInstanceId>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct State {
    pub(crate) traces: BTreeMap<TraceId, TraceInfo>,
    pub(crate) services: BTreeMap<ServiceKey, ServiceState>,
    pub(crate) last_span: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TraceInfo {
    pub(crate) last_seen: DateTime<Utc>,
    pub(crate) spans: BTreeMap<SpanId, SpanInfo>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub(crate) struct SpanInfo {
    pub(crate) key: Option<OperationKey>,
    #[serde(default)]
    pub(crate) parent_of: Vec<OperationKey>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct OperationKey {
    pub(crate) service_key: ServiceKey,
    pub(crate) operation_name: OperationName,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ServiceState {
    pub(crate) id: Uuid,
    #[serde(default)]
    pub(crate) meta: ServiceMeta,
    pub(crate) relations: BTreeMap<ServiceKey, RelationState>,
    pub(crate) operations: BTreeMap<OperationName, OperationState>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct OperationState {
    pub(crate) id: Uuid,
    pub(crate) relations: BTreeMap<ServiceKey, BTreeMap<OperationName, RelationState>>,
    pub(crate) last_seen: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct RelationState {
    pub(crate) id: Uuid,
    pub(crate) last_seen: DateTime<Utc>,
}

impl State {
    pub(crate) fn new() -> Self {
        State::default()
    }
}

impl Display for ServiceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ns) = &self.namespace {
            write!(f, "{ns}/")?;
        }
        write!(f, "{}", self.name)?;
        if let Some(inst) = &self.instance_id {
            write!(f, " {inst}")?;
        }
        Ok(())
    }
}

impl Display for ServiceNamespace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for ServiceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for ServiceInstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ServiceKey {
    type Err = Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (namespace, s) = s.split_once('/').map_or((None, s), |(ns, s)| {
            (Some(ServiceNamespace(ns.to_string())), s)
        });
        let (name, instance_id) = s.split_once(' ').map_or_else(
            || (ServiceName(s.to_string()), None),
            |(name, id)| {
                (
                    ServiceName(name.to_string()),
                    Some(ServiceInstanceId(id.to_string())),
                )
            },
        );
        Ok(ServiceKey {
            namespace,
            name,
            instance_id,
        })
    }
}
