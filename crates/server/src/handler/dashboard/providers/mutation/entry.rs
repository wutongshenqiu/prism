use super::request::{CreateProviderRequest, UpdateProviderRequest};
use prism_core::auth_profile::{AuthProfileEntry, OAuthTokenState};
use prism_core::config::{ModelMapping, ProviderKeyEntry};
use prism_core::presentation::UpstreamPresentationConfig;
use prism_core::provider::{Format, UpstreamKind, WireApi};

pub(super) struct PreparedProviderUpdate {
    pub candidate_entry: ProviderKeyEntry,
    pub auth_profiles_for_write: Option<Vec<AuthProfileEntry>>,
    pub runtime_oauth_states: Vec<(String, OAuthTokenState)>,
}

fn model_mappings(models: &[String]) -> Vec<ModelMapping> {
    models
        .iter()
        .map(|id| ModelMapping {
            id: id.clone(),
            alias: None,
        })
        .collect()
}

fn resolve_wire_api(
    upstream: UpstreamKind,
    wire_api: Option<&str>,
    existing: Option<WireApi>,
) -> WireApi {
    if upstream == UpstreamKind::Codex {
        WireApi::Responses
    } else {
        match wire_api {
            Some("responses") => WireApi::Responses,
            Some(_) => WireApi::Chat,
            None => existing.unwrap_or(WireApi::Chat),
        }
    }
}

pub(super) fn create_provider_entry(
    body: &CreateProviderRequest,
    format: Format,
    upstream: UpstreamKind,
    auth_profiles: Vec<AuthProfileEntry>,
) -> ProviderKeyEntry {
    ProviderKeyEntry {
        name: body.name.clone(),
        format,
        upstream: Some(upstream),
        api_key: body.api_key.clone().unwrap_or_default(),
        base_url: body.base_url.clone(),
        proxy_url: body.proxy_url.clone(),
        prefix: body.prefix.clone(),
        models: model_mappings(&body.models),
        excluded_models: body.excluded_models.clone(),
        headers: body.headers.clone(),
        disabled: body.disabled,
        cloak: Default::default(),
        upstream_presentation: body.upstream_presentation.clone().unwrap_or_default(),
        wire_api: resolve_wire_api(upstream, body.wire_api.as_deref(), None),
        weight: body.weight,
        region: body.region.clone(),
        credential_source: None,
        auth_profiles,
        vertex: body.vertex,
        vertex_project: body.vertex_project.clone(),
        vertex_location: body.vertex_location.clone(),
    }
}

pub(super) fn prepare_provider_update(
    existing_entry: &ProviderKeyEntry,
    request: &UpdateProviderRequest,
    upstream: UpstreamKind,
    auth_profiles: Option<Vec<AuthProfileEntry>>,
    strip_runtime_oauth_data: impl FnOnce(
        Vec<AuthProfileEntry>,
    )
        -> (Vec<AuthProfileEntry>, Vec<(String, OAuthTokenState)>),
) -> PreparedProviderUpdate {
    let mut candidate_entry = existing_entry.clone();
    candidate_entry.upstream = Some(upstream);

    if let Some(ref key) = request.api_key {
        candidate_entry.api_key = key.clone();
    }
    if let Some(ref profiles) = auth_profiles {
        candidate_entry.auth_profiles = profiles.clone();
        if !profiles.is_empty() && request.api_key.is_none() {
            candidate_entry.api_key.clear();
        }
    }
    if let Some(ref url) = request.base_url {
        candidate_entry.base_url = url.clone();
    }
    if let Some(ref url) = request.proxy_url {
        candidate_entry.proxy_url = url.clone();
    }
    if let Some(ref prefix) = request.prefix {
        candidate_entry.prefix = prefix.clone();
    }
    if let Some(ref models) = request.models {
        candidate_entry.models = model_mappings(models);
    }
    if let Some(ref excluded) = request.excluded_models {
        candidate_entry.excluded_models = excluded.clone();
    }
    if let Some(ref headers) = request.headers {
        candidate_entry.headers = headers.clone();
    }
    if let Some(disabled) = request.disabled {
        candidate_entry.disabled = disabled;
    }
    candidate_entry.wire_api = resolve_wire_api(
        upstream,
        request.wire_api.as_ref().and_then(|value| value.as_deref()),
        Some(candidate_entry.wire_api),
    );
    if let Some(weight) = request.weight {
        candidate_entry.weight = weight;
    }
    if let Some(ref region) = request.region {
        candidate_entry.region = region.clone();
    }
    if let Some(ref presentation_opt) = request.upstream_presentation {
        candidate_entry.upstream_presentation = presentation_opt
            .clone()
            .unwrap_or_else(UpstreamPresentationConfig::default);
    }
    if let Some(vertex) = request.vertex {
        candidate_entry.vertex = vertex;
    }
    if let Some(ref project) = request.vertex_project {
        candidate_entry.vertex_project = project.clone();
    }
    if let Some(ref location) = request.vertex_location {
        candidate_entry.vertex_location = location.clone();
    }

    let runtime_oauth_states = auth_profiles.map(strip_runtime_oauth_data);

    PreparedProviderUpdate {
        candidate_entry,
        auth_profiles_for_write: runtime_oauth_states
            .as_ref()
            .map(|(profiles, _)| profiles.clone()),
        runtime_oauth_states: runtime_oauth_states
            .map(|(_, states)| states)
            .unwrap_or_default(),
    }
}

pub(super) fn apply_provider_update(
    entry: &mut ProviderKeyEntry,
    request: &UpdateProviderRequest,
    auth_profiles_for_write: Option<&Vec<AuthProfileEntry>>,
) {
    if let Some(ref key) = request.api_key {
        entry.api_key = key.clone();
    }
    if let Some(ref upstream_opt) = request.upstream {
        entry.upstream = upstream_opt
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .and_then(|value| value.parse().ok());
    }
    if let Some(profiles) = auth_profiles_for_write {
        entry.auth_profiles = profiles.clone();
        if !profiles.is_empty() && request.api_key.is_none() {
            entry.api_key.clear();
        }
    }
    if let Some(ref url) = request.base_url {
        entry.base_url = url.clone();
    }
    if let Some(ref url) = request.proxy_url {
        entry.proxy_url = url.clone();
    }
    if let Some(ref prefix) = request.prefix {
        entry.prefix = prefix.clone();
    }
    if let Some(ref models) = request.models {
        entry.models = model_mappings(models);
    }
    if let Some(ref excluded) = request.excluded_models {
        entry.excluded_models = excluded.clone();
    }
    if let Some(ref headers) = request.headers {
        entry.headers = headers.clone();
    }
    if let Some(disabled) = request.disabled {
        entry.disabled = disabled;
    }
    entry.wire_api = resolve_wire_api(
        entry.upstream_kind(),
        request.wire_api.as_ref().and_then(|value| value.as_deref()),
        Some(entry.wire_api),
    );
    if let Some(weight) = request.weight {
        entry.weight = weight;
    }
    if let Some(ref region) = request.region {
        entry.region = region.clone();
    }
    if let Some(ref presentation_opt) = request.upstream_presentation {
        entry.upstream_presentation = presentation_opt
            .clone()
            .unwrap_or_else(UpstreamPresentationConfig::default);
    }
    if let Some(vertex) = request.vertex {
        entry.vertex = vertex;
    }
    if let Some(ref project) = request.vertex_project {
        entry.vertex_project = project.clone();
    }
    if let Some(ref location) = request.vertex_location {
        entry.vertex_location = location.clone();
    }
}
