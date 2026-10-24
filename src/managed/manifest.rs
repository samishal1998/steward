use super::*;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedPlan {
    schema_version: String,
    kind: String,
    applicable: bool,
    owner: String,
    state_fingerprint: String,
    steps: Vec<Step>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Step {
    proxy: String,
    routes: Vec<RouteSpec>,
    source_fingerprint: String,
    native_fingerprint: String,
}

impl Store {
    pub fn plan_manifest(
        &self,
        manifest: &crate::model::Manifest,
        output: Option<&Path>,
    ) -> Result<Value> {
        self.ready()?;
        if manifest.scope != Scope::User || !manifest.services.is_empty() {
            return Err(error(
                "plan.scope",
                "Route plans require user scope; service composition uses up/down",
            ));
        }
        let mut groups = std::collections::BTreeMap::<String, Vec<RouteSpec>>::new();
        for spec in &manifest.routes {
            groups
                .entry(spec.proxy.clone())
                .or_default()
                .push(spec.clone());
        }
        let mut steps = Vec::new();
        for (proxy, routes) in groups {
            let (prepared, _) = self.prepare_edit(
                &proxy,
                Edit::Upsert {
                    routes: routes.clone(),
                },
            )?;
            let native = transport::fetch(&prepared.connection.binding)?;
            steps.push(Step {
                proxy,
                routes,
                source_fingerprint: fingerprint(&prepared.before),
                native_fingerprint: fingerprint(&native.values),
            });
        }
        let plan = SavedPlan {
            schema_version: "1".into(),
            kind: "steward.route-plan".into(),
            applicable: true,
            owner: self.book.owner.clone(),
            state_fingerprint: fingerprint(&self.book),
            steps,
        };
        if let Some(path) = output {
            absolute(path)?;
            if regular(path)?.is_some() {
                return Err(error(
                    "plan.exists",
                    "Plan output already exists; choose a new filename",
                ));
            }
            publish(path, Some(&encoded(&plan)?))?;
        }
        Ok(json!(plan))
    }

    pub fn apply_manifest(&mut self, path: &Path) -> Result<Value> {
        self.ready()?;
        let plan: SavedPlan = config::load(path, config::MAX_CONFIG)?;
        config::version(&plan.schema_version)?;
        if plan.kind != "steward.route-plan"
            || !plan.applicable
            || plan.owner != self.book.owner
            || plan.state_fingerprint != fingerprint(&self.book)
            || plan.steps.len() > 16
        {
            return Err(error(
                "plan.stale",
                "Plan kind, owner, or current state does not match; create a fresh plan",
            ));
        }
        let mut proxies = std::collections::BTreeSet::new();
        // Revalidate every step before any write. Each commit repeats source/native
        // preconditions immediately before publication; multi-proxy effects are explicit.
        for step in &plan.steps {
            if !proxies.insert(&step.proxy) {
                return Err(error("plan.duplicate", "Duplicate proxy step"));
            }
            self.check_step(step)?;
        }
        let mut operations = Vec::new();
        for step in &plan.steps {
            let result = self
                .check_step(step)
                .and_then(|prepared| self.commit_edit(prepared, "manifest"));
            match result {
                Ok(value) => operations.push(value),
                Err(mut e) => {
                    if !operations.is_empty() {
                        e.changed = true;
                        e.message = format!(
                            "{}; earlier operations remain applied: {}",
                            e.message,
                            operations
                                .iter()
                                .filter_map(|v| v["operation"].as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    return Err(e);
                }
            }
        }
        Ok(json!({"operations":operations,"pruned":false}))
    }

    fn check_step(&self, step: &Step) -> Result<Journal> {
        let (prepared, _) = self.prepare_edit(
            &step.proxy,
            Edit::Upsert {
                routes: step.routes.clone(),
            },
        )?;
        if fingerprint(&prepared.before) != step.source_fingerprint
            || fingerprint(&transport::fetch(&prepared.connection.binding)?.values)
                != step.native_fingerprint
        {
            return Err(error(
                "plan.stale",
                "Authoritative input or native configuration changed since planning",
            )
            .at(&step.proxy));
        }
        Ok(prepared)
    }
}
