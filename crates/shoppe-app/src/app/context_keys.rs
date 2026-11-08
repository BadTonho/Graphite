use super::editor_groups::ViewId;
use std::collections::HashMap;

/// Context values scoped to the editor view, with shared global defaults.
#[derive(Default)]
pub(crate) struct ContextKeyService {
    global: HashMap<String, String>,
    views: HashMap<ViewId, HashMap<String, String>>,
}

impl ContextKeyService {
    pub(super) fn replace_global(&mut self, values: HashMap<String, String>) {
        self.global = values;
    }

    pub(super) fn replace_view(&mut self, view: ViewId, values: HashMap<String, String>) {
        self.views.insert(view, values);
    }

    pub(super) fn retain_views(&mut self, live: &std::collections::HashSet<ViewId>) {
        self.views.retain(|id, _| live.contains(id));
    }

    pub(super) fn resolve(&self, target: Option<ViewId>) -> HashMap<String, String> {
        let mut resolved = self.global.clone();
        if let Some(values) = target.and_then(|id| self.views.get(&id)) {
            resolved.extend(values.clone());
        }
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::editor_groups::EditorGroups;
    use crate::DocumentId;

    #[test]
    fn view_values_override_global_and_do_not_leak_to_other_views() {
        let mut groups = EditorGroups::new(DocumentId(1));
        let first = groups.active_view().id;
        groups.split(super::super::editor_groups::SplitDirection::Vertical);
        let second = groups.active_view().id;
        let mut service = ContextKeyService::default();
        service.replace_global(HashMap::from([("key".into(), "global".into())]));
        service.replace_view(first, HashMap::from([("key".into(), "first".into())]));
        assert_eq!(service.resolve(Some(first))["key"], "first");
        assert_eq!(service.resolve(Some(second))["key"], "global");
        assert_eq!(service.resolve(None)["key"], "global");
        service.retain_views(&std::collections::HashSet::from([second]));
        assert_eq!(service.resolve(Some(first))["key"], "global");
    }
}
