use super::*;

pub(super) fn adapt(binary: &Path, file: &Path, input: &str) -> Result<Value> {
    let mut stage = tempfile::NamedTempFile::new_in(
        file.parent()
            .ok_or_else(|| error("authority.file", "Missing Caddyfile parent"))?,
    )
    .map_err(io_error)?;
    stage.write_all(input.as_bytes()).map_err(io_error)?;
    let bytes = crate::executor::checked(
        binary,
        &[
            "adapt".as_ref(),
            "--config".as_ref(),
            stage.path().as_os_str(),
            "--adapter".as_ref(),
            "caddyfile".as_ref(),
        ],
    )?;
    config::strict_json(&bytes)
}

pub(super) fn native(connection: &Connection, input: &str) -> Result<Value> {
    let Authority::CaddyFile { file, binary, .. } = &connection.authority else {
        return Err(error("authority.kind", "Expected Caddyfile authority"));
    };
    adapt(binary, file, input)
}

fn block(connection: &Connection, book: &Book) -> Result<String> {
    let Authority::CaddyFile { listen, .. } = &connection.authority else {
        unreachable!()
    };
    let port = listener(listen)?
        .rsplit(':')
        .next()
        .unwrap_or("")
        .to_owned();
    let mut output = format!("# steward {} {} begin\n", book.owner, connection.binding.id);
    for receipt in own(book, &connection.binding.id)
        .iter()
        .filter(|r| r.spec.enabled)
    {
        let spec = &receipt.spec;
        // Identifiers/host/origin are semantically validated before interpolation.
        let scheme = if matches!(spec.tls.mode, TlsMode::Http) {
            "http"
        } else {
            "https"
        };
        output.push_str(&format!("{scheme}://{}:{port} {{\n", spec.hostname));
        let host = listen.rsplit_once(':').map(|(h, _)| h).unwrap_or("");
        if !host.is_empty() {
            output.push_str(&format!("  bind {host}\n"));
        }
        if matches!(spec.tls.mode, TlsMode::Internal) {
            output.push_str("  tls internal\n");
        }
        output.push_str(&format!(
            "  reverse_proxy {}\n}}\n",
            serde_json::to_string(&spec.upstream.url).expect("origin")
        ));
    }
    output.push_str(&format!(
        "# steward {} {} end\n",
        book.owner, connection.binding.id
    ));
    Ok(output)
}

pub(super) fn candidate(
    connection: &Connection,
    old: &Book,
    desired: &Book,
    input: Option<&str>,
) -> Result<Option<String>> {
    let mut source = input
        .ok_or_else(|| error("authority.file", "Caddyfile disappeared"))?
        .to_owned();
    let marker = format!("# steward {} {} begin\n", old.owner, connection.binding.id);
    if let Some(start) = source.find(&marker) {
        let expected = block(connection, old)?;
        if !source[start..].starts_with(&expected) || source.matches(&marker).count() != 1 {
            return Err(error(
                "ownership.drift",
                "Managed Caddyfile section differs from its receipt",
            ));
        }
        source.replace_range(start..start + expected.len(), "");
    } else if !own(old, &connection.binding.id).is_empty() {
        return Err(error(
            "ownership.drift",
            "Managed Caddyfile section disappeared",
        ));
    }
    if !source.ends_with('\n') {
        source.push('\n');
    }
    if !own(desired, &connection.binding.id).is_empty() {
        source.push_str(&block(connection, desired)?);
    }
    Ok(Some(source))
}

pub(super) fn route_matches(
    connection: &Connection,
    observed: &crate::model::RouteObservation,
    receipt: &Receipt,
) -> bool {
    let Authority::CaddyFile { listen, .. } = &connection.authority else {
        return false;
    };
    let Ok(url) = config::http_url(&receipt.spec.upstream.url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let Some(port) = url.port_or_known_default() else {
        return false;
    };
    observed.entrypoints.contains(listen)
        && caddy_route(receipt)
            .is_ok_and(|expected| observed.native["handler"] == expected["handle"][0])
        && observed.matcher_coverage == "literal_subset"
        && observed.hosts == [receipt.spec.hostname.clone()]
        && observed.targets == [format!("{host}:{port}")]
        && observed.paths.is_empty()
        && !observed.any_host
}
