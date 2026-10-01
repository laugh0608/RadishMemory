//! Adapter-private request binding, versioned independently of canonical identity.
use crate::source_store::{
    deletion_state_str, egress_policy_str, producer_type_str, retention_mode_str, sensitivity_str,
    source_kind_str, source_origin_kind_str,
};
use radishmemory_core::{Governance, ProducerRef, SourceCapture, compute_exact_bytes_digest};

pub(crate) fn fingerprint(capture: &SourceCapture) -> String {
    let mut codec = Codec(b"radishmemory.capture-attempt/1\0".to_vec());
    codec.text(capture.origin_binding_id().as_str());
    let p = capture.source().params();
    for s in [
        p.source_id.as_str(),
        p.lineage_id.as_str(),
        p.namespace_id.as_str(),
        source_kind_str(p.source_kind),
        p.media_type.as_str(),
        p.content_digest.algorithm(),
        p.content_digest.profile().as_str(),
        p.content_digest.value(),
        source_origin_kind_str(p.origin_kind),
        p.observed_at.original(),
        p.captured_at.original(),
        p.created_at.original(),
    ] {
        codec.text(s);
    }
    codec.number(p.version.get());
    codec.number(p.content_length);
    codec.optional(p.title.as_ref().map(|v| v.as_str()));
    codec.optional(p.origin_ref.as_ref().map(|v| v.as_str()));
    codec.number(p.supersedes_source_ids.len() as u64);
    for id in &p.supersedes_source_ids {
        codec.text(id.as_str());
    }
    codec.governance(&p.governance);
    codec.producer(&p.producer);
    // Fragment content is already validated against its digest and source range.
    let mut fragments: Vec<_> = capture.fragments().iter().collect();
    fragments.sort_by_key(|f| f.params().ordinal);
    codec.number(fragments.len() as u64);
    for fragment in fragments {
        let p = fragment.params();
        for s in [
            p.fragment_id.as_str(),
            p.namespace_id.as_str(),
            p.source_id.as_str(),
            p.content_digest.algorithm(),
            p.content_digest.profile().as_str(),
            p.content_digest.value(),
            p.created_at.original(),
        ] {
            codec.text(s);
        }
        codec.number(p.ordinal);
        codec.number(p.byte_start);
        codec.number(p.byte_end);
        match &p.heading_path {
            None => codec.number(0),
            Some(headings) => {
                codec.number(1);
                codec.number(headings.len() as u64);
                for h in headings {
                    codec.text(h.as_str());
                }
            }
        }
        codec.governance(&p.governance);
        codec.producer(&p.segmenter);
    }
    compute_exact_bytes_digest(&codec.0).value().to_owned()
}
struct Codec(Vec<u8>);
impl Codec {
    fn number(&mut self, n: u64) {
        self.0.extend_from_slice(&n.to_be_bytes());
    }
    fn text(&mut self, text: &str) {
        self.number(text.len() as u64);
        self.0.extend_from_slice(text.as_bytes());
    }
    fn optional(&mut self, text: Option<&str>) {
        match text {
            None => self.number(0),
            Some(s) => {
                self.number(1);
                self.text(s);
            }
        }
    }
    fn governance(&mut self, g: &Governance) {
        for s in [
            sensitivity_str(g.sensitivity()),
            egress_policy_str(g.egress_policy()),
            retention_mode_str(g.retention().mode()),
            deletion_state_str(g.deletion_state()),
            g.policy_basis().as_str(),
        ] {
            self.text(s);
        }
        self.optional(g.retention().expires_at().map(|t| t.original()));
        self.optional(g.retention().policy_id().map(|v| v.as_str()));
    }
    fn producer(&mut self, p: &ProducerRef) {
        for s in [
            producer_type_str(p.producer_type()),
            p.producer_id().as_str(),
            p.producer_version().as_str(),
        ] {
            self.text(s);
        }
    }
}
