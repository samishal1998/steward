use super::*;

const MAX_ENTRIES: usize = 1024;
const MAX_BYTES: u64 = 256 * 1024 * 1024;

fn summary(j: &Journal) -> Value {
    json!({"id":j.id,"phase":j.phase,"proxy":j.connection.binding.id,"reverses":j.reverses,
        "before_fingerprint":fingerprint(&j.before),"after_fingerprint":fingerprint(&j.after),
        "routes_before":own(&j.old,&j.connection.binding.id).iter().map(|r| &r.spec.id).collect::<Vec<_>>(),
        "routes_after":own(&j.desired,&j.connection.binding.id).iter().map(|r| &r.spec.id).collect::<Vec<_>>()})
}

impl Store {
    fn history_files(&self) -> Result<Vec<PathBuf>> {
        let directory = self.root.join("operations");
        if !directory.exists() {
            return Ok(Vec::new());
        }
        if fs::symlink_metadata(&directory)
            .map_err(io_error)?
            .file_type()
            .is_symlink()
        {
            return Err(error(
                "state.type",
                "History directory must not be a symlink",
            ));
        }
        let mut paths = Vec::new();
        let mut bytes = 0;
        for entry in fs::read_dir(directory).map_err(io_error)? {
            let path = entry.map_err(io_error)?.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let id = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");
            operation_id(id)?;
            bytes += regular(&path)?
                .ok_or_else(|| error("state.history", "History file disappeared"))?
                .len();
            paths.push(path);
            if paths.len() > MAX_ENTRIES || bytes > MAX_BYTES {
                return Err(error(
                    "state.history_limit",
                    "History exceeds 1024 operations or 256 MiB; archive entries before continuing",
                ));
            }
        }
        Ok(paths)
    }

    pub(super) fn prepare_history(&self, journal: &Journal) -> Result<()> {
        operation_id(&journal.id)?;
        let files = self.history_files()?;
        let directory = self.root.join("operations");
        if !directory.exists() {
            fs::create_dir(&directory).map_err(io_error)?;
        }
        if !directory.join(format!("{}.json", journal.id)).exists() && files.len() >= MAX_ENTRIES {
            return Err(error(
                "state.history_limit",
                "1024 operations retained; archive history before starting another operation",
            ));
        }
        let bytes: u64 = files
            .iter()
            .filter(|p| p.file_stem().and_then(|s| s.to_str()) != Some(&journal.id))
            .map(|p| fs::metadata(p).map(|m| m.len()).map_err(io_error))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .sum();
        if bytes + encoded(journal)?.len() as u64 > MAX_BYTES {
            return Err(error(
                "state.history_limit",
                "History would exceed 256 MiB; archive entries before starting another operation",
            ));
        }
        Ok(())
    }

    pub fn operations(&self) -> Result<Value> {
        let active = self.journal()?;
        let mut entries = Vec::new();
        if let Some(j) = &active {
            entries.push(summary(j));
        }
        let mut files = self.history_files()?;
        files.sort();
        for file in files {
            if active
                .as_ref()
                .is_some_and(|j| file.file_stem().and_then(|s| s.to_str()) == Some(&j.id))
            {
                continue;
            }
            let journal: Journal = config::load(&file, 8 * config::MAX_CONFIG)?;
            entries.push(summary(&journal));
        }
        entries.extend(crate::services::operation_summaries(&self.root)?);
        Ok(json!(entries))
    }

    fn history_entry(&self, id: &str) -> Result<Journal> {
        operation_id(id)?;
        if let Some(active) = self.journal()?
            && active.id == id
        {
            return Ok(active);
        }
        let file = self.root.join("operations").join(format!("{id}.json"));
        regular(&file)?.ok_or_else(|| error("operation.missing", "Operation was not found"))?;
        config::load(&file, 8 * config::MAX_CONFIG)
    }

    pub fn inspect_operation(&self, id: &str) -> Result<Value> {
        operation_id(id)?;
        if let Some(service) = crate::services::operation_summaries(&self.root)?
            .into_iter()
            .find(|j| j["id"] == id)
        {
            return Ok(service);
        }
        Ok(summary(&self.history_entry(id)?))
    }

    pub fn rollback(&mut self, id: &str) -> Result<Value> {
        self.ready()?;
        let original = self.history_entry(id)?;
        if original.phase != "verified" {
            return Err(error(
                "operation.terminal",
                "Only a verified operation can be rolled back; use recover for unfinished writes",
            ));
        }
        let _input_lock = input_lock(&original.connection)?;
        if fingerprint(&self.book) != fingerprint(&original.desired)
            || text(&original.source)? != original.after
        {
            return Err(error(
                "operation.drift",
                "State or source changed after this operation; roll back later operations first",
            ));
        }
        if !native_matches(
            &original.connection,
            &original.desired,
            original.after.as_deref(),
        )? {
            return Err(error(
                "operation.drift",
                "Runtime no longer matches the operation being reversed",
            ));
        }
        let operation = fingerprint(&(
            id,
            transport::now(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        let mut reverse = Journal {
            native_before: original.native_after,
            native_after: original.native_before,
            id: operation[..16].into(),
            phase: "prepared".into(),
            reverses: Some(id.into()),
            before: original.after,
            after: original.before,
            old: self.book.clone(),
            desired: original.old,
            schema_version: "1".into(),
            connection: original.connection,
            source: original.source,
        };
        if let Authority::CaddyJson { binary, .. } = &reverse.connection.authority {
            validate_caddy(
                binary,
                &reverse.source,
                reverse
                    .after
                    .as_deref()
                    .ok_or_else(|| error("state.journal", "Missing source backup"))?,
            )?;
        }
        self.record(&reverse)?;
        if let Err(e) = self.execute(&mut reverse) {
            if self.compensate(&mut reverse).is_ok() {
                return Err(error(
                    "operation.compensated",
                    &format!("Rollback failed ({}); preceding state restored", e.code),
                )
                .at(reverse.id));
            }
            return Err(error(
                "operation.recovery_required",
                "Rollback interrupted; inspect/recover its operation",
            )
            .at(reverse.id)
            .with_changes());
        }
        self.book = reverse.desired.clone();
        Ok(summary(&reverse))
    }
}

fn operation_id(id: &str) -> Result<()> {
    if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(error(
            "operation.id",
            "Operation IDs are 16 hexadecimal characters",
        ));
    }
    Ok(())
}
