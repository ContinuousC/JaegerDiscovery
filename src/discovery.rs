/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    num::ParseIntError,
    path::PathBuf,
    str::FromStr,
};

use chrono::{DateTime, TimeDelta, Utc};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use url::Url;
use uuid::Uuid;

use crate::{
    error::Error,
    load_cert, load_identity, load_json,
    query::EsPit,
    save_json,
    state::{
        OperationKey, OperationName, OperationState, ServiceInstanceId, ServiceKey, ServiceName,
        ServiceNamespace, ServiceState, SpanId, State, TraceId, TraceInfo,
    },
    Args,
};

pub(crate) struct Discovery {
    state_path: PathBuf,
    state: State,
    rg_client: Client,
    es_client: Client,
    es_url: Url,
    rg_url: Url,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Service {
    pub(crate) service_name: ServiceName,
    pub(crate) operation_name: OperationName,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Span {
    #[serde(rename = "traceID")]
    pub(crate) trace_id: TraceId,
    #[serde(rename = "spanID")]
    pub(crate) span_id: SpanId,
    pub(crate) operation_name: OperationName,
    pub(crate) references: Vec<Reference>,
    pub(crate) start_time: i64,
    pub(crate) start_time_millis: i64,
    pub(crate) duration: u64,
    pub(crate) tags: Vec<Tag>,
    pub(crate) logs: Vec<Log>,
    pub(crate) process: Process,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Reference {
    pub(crate) ref_type: RefType,
    #[serde(rename = "traceID")]
    pub(crate) trace_id: TraceId,
    #[serde(rename = "spanID")]
    pub(crate) span_id: SpanId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RefType {
    ChildOf,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tag {
    pub(crate) key: String,
    #[serde(flatten)]
    pub(crate) value: TagValue, // pub(crate) r#type: TagType,
                                // pub(crate) value: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TagType {
    String,
    Int64,
    Bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub(crate) enum TagValue {
    String(String),
    Int64(Int64),
    Bool(Bool),
}

#[derive(SerializeDisplay, DeserializeFromStr, Debug)]
pub(crate) struct Int64(i64);

impl Display for Int64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Int64 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Bool {
    True,
    False,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Log {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Process {
    pub(crate) service_name: ServiceName,
    pub(crate) tags: Vec<Tag>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Items {
    pub(crate) domain: Domain,
    pub(crate) items: World,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Domain {
    pub roots: Option<BTreeSet<Uuid>>,
    pub types: TypeSet,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TypeSet {
    pub items: BTreeSet<String>,
    pub relations: BTreeSet<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct World {
    pub(crate) items: BTreeMap<Uuid, Item>,
    pub(crate) relations: BTreeMap<Uuid, Relation>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "item_type")]
pub(crate) enum Item {
    #[serde(rename = "jaeger/service")]
    Service { properties: Box<ServiceProps> },
    #[serde(rename = "jaeger/operation")]
    Operation {
        parent: Uuid,
        properties: Box<OperationProps>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ServiceProps {
    #[serde(
        default,
        rename = "jaeger/service_namespace",
        skip_serializing_if = "Option::is_none"
    )]
    service_namespace: Option<StringProperty<ServiceNamespace>>,
    #[serde(rename = "jaeger/service_name")]
    service_name: StringProperty<ServiceName>,
    #[serde(
        default,
        rename = "jaeger/service_instance_id",
        skip_serializing_if = "Option::is_none"
    )]
    service_instance_id: Option<StringProperty<ServiceInstanceId>>,
    #[serde(flatten)]
    meta: ServiceMeta,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub(crate) struct ServiceMeta {
    #[serde(
        default,
        rename = "jaeger/service_version",
        skip_serializing_if = "Option::is_none"
    )]
    service_version: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/deployment_environment",
        skip_serializing_if = "Option::is_none"
    )]
    deployment_environment: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_cluster_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_cluster_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_cluster_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_cluster_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_node_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_node_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_node_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_node_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_namespace_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_namespace_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_pod_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_pod_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_pod_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_pod_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_container_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_container_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_replicaset_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_replicaset_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_replicaset_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_replicaset_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_deployment_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_deployment_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_deployment_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_deployment_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_statefulset_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_statefulset_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_statefulset_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_statefulset_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_daemonset_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_daemonset_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_daemonset_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_daemonset_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_job_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_job_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_job_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_job_uid: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_cronjob_name",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_cronjob_name: Option<StringProperty>,
    #[serde(
        default,
        rename = "jaeger/k8s_cronjob_uid",
        skip_serializing_if = "Option::is_none"
    )]
    k8s_cronjob_uid: Option<StringProperty>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct OperationProps {
    #[serde(rename = "jaeger/operation_name")]
    operation_name: StringProperty<OperationName>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "relation_type")]
pub(crate) enum Relation {
    #[serde(rename = "jaeger/service_invokes")]
    ServiceInvokes {
        source: Uuid,
        target: Uuid,
        properties: InvokesProps,
    },
    #[serde(rename = "jaeger/operation_invokes")]
    OperationInvokes {
        source: Uuid,
        target: Uuid,
        properties: InvokesProps,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct InvokesProps {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct StringProperty<T = String> {
    string: T,
}

impl<T> StringProperty<T> {
    fn new(string: T) -> StringProperty<T> {
        Self { string }
    }
}

impl Discovery {
    pub(crate) async fn new(args: &Args) -> Result<Self, Error> {
        let state_path = args.state.join("state.json.gz");
        let state = if state_path.exists() {
            load_json::<State>(&state_path).await?
        } else {
            State::new()
        };

        let mut headers = HeaderMap::new();
        headers.insert("X-PROXY-ROLE", HeaderValue::try_from("Editor").unwrap());

        let rg_client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .default_headers(headers)
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true) // TODO: disable
            .build()
            .map_err(Error::Reqwest)?;
        let es_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .add_root_certificate(load_cert(&args.es_ca).await?)
            .identity(load_identity(&args.es_cert, &args.es_key).await?)
            .danger_accept_invalid_hostnames(true) // TODO: disable!
            .build()
            .map_err(Error::Reqwest)?;
        let es_url = args.es_url.clone();
        let rg_url = args.rg_url.clone();

        Ok(Self {
            state_path,
            state,
            rg_client,
            es_client,
            es_url,
            rg_url,
        })
    }

    pub(crate) async fn discover(&mut self) -> Result<(), Error> {
        log::info!("running discovery");

        let now = Utc::now();
        let oper_threshold = now - TimeDelta::try_days(7).unwrap();

        let mut pit = EsPit::new(&self.es_client, &self.es_url, "jaeger-span-*", "1m").await?;
        let mut query = pit.query::<_, serde_json::Value, (i64,), Span>(
            json!({
                "range": {
                    "startTime": {
                        "gte": oper_threshold.timestamp_micros()
                    }
                }
            }),
            Some(json!([{ "startTime": { "order": "asc" } }])),
            self.state.last_span.map(|v| (v.timestamp_micros(),)),
            1000,
        );

        let mut n = 0;
        let res = async {
            while let Some(res) = query.next().await? {
                n += res.hits.hits.len();
                if let Some(last) = res
                    .hits
                    .hits
                    .last()
                    .and_then(|hit| hit.sort.as_ref())
                    .map(|sort| sort.0)
                {
                    self.state.last_span = Some(
                        DateTime::from_timestamp_micros(last)
                            .ok_or(Error::TimestampOutOfBounds(last))?,
                    );
                }

                for hit in res.hits.hits {
                    let span = hit.source;
                    let t = DateTime::from_timestamp_micros(span.start_time)
                        .ok_or(Error::TimestampOutOfBounds(span.start_time))?;

                    /* Find service key.*/

                    let service_key = ServiceKey {
                        namespace: span
                            .process
                            .tags
                            .iter()
                            .filter(|tag| &tag.key == "service.namespace")
                            .find_map(|tag| match &tag.value {
                                TagValue::String(s) => Some(ServiceNamespace(s.to_string())),
                                _ => None,
                            }),
                        name: span.process.service_name.clone(),
                        instance_id: span
                            .process
                            .tags
                            .iter()
                            .filter(|tag| (&tag.key == "service.instance.id"))
                            .find_map(|tag| match &tag.value {
                                TagValue::String(s) => Some(ServiceInstanceId(s.to_string())),
                                _ => None,
                            }),
                    };

                    let svc_meta = ServiceMeta::from_span(&span);

                    /* Insert into trace and span map. */

                    let trace_info = self
                        .state
                        .traces
                        .entry(span.trace_id.clone())
                        .and_modify(|info| info.last_seen = t)
                        .or_insert_with(|| TraceInfo {
                            last_seen: t,
                            spans: BTreeMap::new(),
                        });

                    let span_info = trace_info.spans.entry(span.span_id.clone()).or_default();
                    span_info.key = Some(OperationKey {
                        service_key: service_key.clone(),
                        operation_name: span.operation_name.clone(),
                    });

                    /* Update services and operations.  */

                    let svc_state = self
                        .state
                        .services
                        .entry(service_key.clone())
                        .and_modify(|svc| svc.meta = svc_meta.clone())
                        .or_insert_with(|| ServiceState {
                            id: Uuid::new_v4(),
                            meta: svc_meta.clone(),
                            relations: BTreeMap::new(),
                            operations: BTreeMap::new(),
                        });

                    let oper_state = svc_state
                        .operations
                        .entry(span.operation_name.clone())
                        .and_modify(|state| state.last_seen = t)
                        .or_insert_with(|| OperationState {
                            id: Uuid::new_v4(),
                            relations: BTreeMap::new(),
                            last_seen: t,
                        });

                    /* Update relations. */

                    let parent_of = std::mem::take(&mut span_info.parent_of);

                    if let Some(r) = span
                        .references
                        .iter()
                        .find(|r| r.ref_type == RefType::ChildOf)
                    {
                        let parent_trace = self
                            .state
                            .traces
                            .entry(r.trace_id.clone())
                            .and_modify(|info| info.last_seen = t)
                            .or_insert_with(|| TraceInfo {
                                last_seen: t,
                                spans: BTreeMap::new(),
                            });
                        let parent_span = parent_trace.spans.entry(r.span_id.clone()).or_default();

                        if let Some(parent_key) = &parent_span.key {
                            if parent_key.service_key != service_key {
                                svc_state
                                    .relations
                                    .entry(parent_key.service_key.clone())
                                    .and_modify(|relation| relation.last_seen = t)
                                    .or_insert_with(|| super::state::RelationState {
                                        id: Uuid::new_v4(),
                                        last_seen: t,
                                    });
                            }

                            oper_state
                                .relations
                                .entry(parent_key.service_key.clone())
                                .or_default()
                                .entry(parent_key.operation_name.clone())
                                .and_modify(|relation| relation.last_seen = t)
                                .or_insert_with(|| super::state::RelationState {
                                    id: Uuid::new_v4(),
                                    last_seen: t,
                                });
                        } else {
                            parent_span.parent_of.push(OperationKey {
                                service_key: service_key.clone(),
                                operation_name: span.operation_name.clone(),
                            })
                        }
                    }

                    for child_key in parent_of {
                        if child_key.service_key != service_key {
                            if let Some(svc_state) =
                                self.state.services.get_mut(&child_key.service_key)
                            {
                                svc_state
                                    .relations
                                    .entry(service_key.clone())
                                    .and_modify(|relation| relation.last_seen = t)
                                    .or_insert_with(|| super::state::RelationState {
                                        id: Uuid::new_v4(),
                                        last_seen: t,
                                    });
                            }
                        }

                        if let Some(oper_state) = self
                            .state
                            .services
                            .get_mut(&child_key.service_key)
                            .and_then(|svc_state| {
                                svc_state.operations.get_mut(&child_key.operation_name)
                            })
                        {
                            oper_state
                                .relations
                                .entry(service_key.clone())
                                .or_default()
                                .entry(span.operation_name.clone())
                                .and_modify(|relation| relation.last_seen = t)
                                .or_insert_with(|| super::state::RelationState {
                                    id: Uuid::new_v4(),
                                    last_seen: t,
                                });
                        }
                    }
                }

                /* Cleanup trace and span map. */

                if let Some(last) = self.state.last_span {
                    let trace_threshold = last - TimeDelta::try_seconds(300).unwrap();
                    self.state
                        .traces
                        .retain(|_, info| info.last_seen >= trace_threshold);
                }
            }

            Ok(())
        }
        .await;

        match res {
            Ok(()) => {
                pit.delete().await.unwrap_or_else(|e| log::warn!("{e}"));
                println!("Processed {n} spans");
            }
            Err(e) => {
                pit.delete().await.unwrap_or_else(|e| log::warn!("{e}"));
                return Err(e);
            }
        }

        /* Cleanup services and operations. */

        self.state.services.retain(|_, svc_state| {
            svc_state
                .relations
                .retain(|_, rel| rel.last_seen >= oper_threshold);

            svc_state.operations.retain(|_, oper_state| {
                oper_state.relations.retain(|_, svc_rels| {
                    svc_rels.retain(|_, rel| rel.last_seen >= oper_threshold);
                    !svc_rels.is_empty()
                });

                oper_state.last_seen >= oper_threshold
            });

            !svc_state.operations.is_empty()
        });

        /* Build item and relation map. */

        let items = self
            .state
            .services
            .iter()
            .map(|(svc_key, svc_state)| {
                (
                    svc_state.id,
                    Item::Service {
                        properties: Box::new(ServiceProps {
                            service_namespace: svc_key.namespace.clone().map(StringProperty::new),
                            service_name: StringProperty::new(svc_key.name.clone()),
                            service_instance_id: svc_key
                                .instance_id
                                .clone()
                                .map(StringProperty::new),
                            meta: svc_state.meta.clone(),
                        }),
                    },
                )
            })
            .chain(self.state.services.values().flat_map(|svc_state| {
                svc_state.operations.iter().map(|(oper_name, oper_state)| {
                    (
                        oper_state.id,
                        Item::Operation {
                            parent: svc_state.id,
                            properties: Box::new(OperationProps {
                                operation_name: StringProperty::new(oper_name.clone()),
                            }),
                        },
                    )
                })
            }))
            .collect::<BTreeMap<_, _>>();

        let relations = self
            .state
            .services
            .values()
            .flat_map(|svc_state| {
                svc_state.relations.iter().filter_map(|(parent_svc, rel)| {
                    Some((
                        rel.id,
                        Relation::ServiceInvokes {
                            source: self.state.services.get(parent_svc)?.id,
                            target: svc_state.id,
                            properties: InvokesProps {},
                        },
                    ))
                })
            })
            .chain(self.state.services.values().flat_map(|svc_state| {
                svc_state.operations.values().flat_map(|oper_state| {
                    oper_state
                        .relations
                        .iter()
                        .flat_map(|(parent_svc, oper_rels)| {
                            oper_rels.iter().filter_map(|(parent_oper, rel)| {
                                Some((
                                    rel.id,
                                    Relation::OperationInvokes {
                                        source: self
                                            .state
                                            .services
                                            .get(parent_svc)?
                                            .operations
                                            .get(parent_oper)?
                                            .id,
                                        target: oper_state.id,
                                        properties: InvokesProps {},
                                    },
                                ))
                            })
                        })
                })
            }))
            .collect::<BTreeMap<_, _>>();

        // let items = items
        //     .into_iter()
        //     .filter(|(_, item)| matches!(item, Item::Service { .. }))
        //     .collect::<BTreeMap<_, _>>();

        // let relations = relations
        //     .into_iter()
        //     .filter(|(_, rel)| match rel {
        //         Relation::ServiceInvokes { source, target, .. } => {
        //             items.contains_key(source) && items.contains_key(target)
        //         }
        //         Relation::OperationInvokes { .. } => false,
        //     })
        //     .collect();

        log::info!(
            "Found {} items, {} relations.",
            items.len(),
            relations.len()
        );

        let items = Items {
            domain: Domain {
                // roots: Some(
                //     self.state
                //         .services
                //         .values()
                //         .map(|svc_state| svc_state.id)
                //         .collect(),
                // ),
                roots: None, /* all jaeger objects */
                types: TypeSet {
                    items: BTreeSet::from_iter([
                        String::from("jaeger/service"),
                        String::from("jaeger/operation"),
                    ]),
                    relations: BTreeSet::from_iter([
                        String::from("jaeger/service_invokes"),
                        String::from("jaeger/operation_invokes"),
                    ]),
                },
            },
            items: World { items, relations },
        };

        let res = self
            .rg_client
            .put(self.rg_url.join("items")?)
            .json(&items)
            .send()
            .await?;

        if let Err(err) = res.error_for_status_ref() {
            let msg = res.text().await?;
            return Err(Error::RelationGraph(err, msg));
        }

        save_json(&self.state_path, &self.state).await?;
        Ok(())
    }
}

impl ServiceMeta {
    fn from_span(span: &Span) -> Self {
        let mut props = Self::default();
        span.process
            .tags
            .iter()
            .for_each(|tag| match (tag.key.as_str(), &tag.value) {
                ("service.version", TagValue::String(s)) => {
                    props.service_version = Some(StringProperty::new(s.to_string()))
                }
                ("deployment.environment", TagValue::String(s)) => {
                    props.deployment_environment = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.cluster.name", TagValue::String(s)) => {
                    props.k8s_cluster_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.cluster.uid", TagValue::String(s)) => {
                    props.k8s_cluster_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.node.name", TagValue::String(s)) => {
                    props.k8s_node_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.node.uid", TagValue::String(s)) => {
                    props.k8s_node_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.namespace.name", TagValue::String(s)) => {
                    props.k8s_namespace_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.pod.name", TagValue::String(s)) => {
                    props.k8s_pod_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.pod.uid", TagValue::String(s)) => {
                    props.k8s_pod_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.container.name", TagValue::String(s)) => {
                    props.k8s_container_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.replicaset.name", TagValue::String(s)) => {
                    props.k8s_replicaset_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.replicaset.uid", TagValue::String(s)) => {
                    props.k8s_replicaset_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.deployment.name", TagValue::String(s)) => {
                    props.k8s_deployment_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.deployment.uid", TagValue::String(s)) => {
                    props.k8s_deployment_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.statefulset.name", TagValue::String(s)) => {
                    props.k8s_statefulset_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.statefulset.uid", TagValue::String(s)) => {
                    props.k8s_statefulset_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.daemonset.name", TagValue::String(s)) => {
                    props.k8s_daemonset_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.daemonset.uid", TagValue::String(s)) => {
                    props.k8s_daemonset_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.job.name", TagValue::String(s)) => {
                    props.k8s_job_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.job.uid", TagValue::String(s)) => {
                    props.k8s_job_uid = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.cronjob.name", TagValue::String(s)) => {
                    props.k8s_cronjob_name = Some(StringProperty::new(s.to_string()))
                }
                ("k8s.cronjob.uid", TagValue::String(s)) => {
                    props.k8s_cronjob_uid = Some(StringProperty::new(s.to_string()))
                }
                _ => {}
            });
        props
    }
}
