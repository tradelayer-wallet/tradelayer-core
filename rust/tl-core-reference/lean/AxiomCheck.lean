import TlCoreRef

open TlCoreRef

-- varint
#print axioms readVarint_encodeVarint
#print axioms encodeVarint_injective
#print axioms readVarint_truncated
#print axioms encodeVarint_bytes_lt
#print axioms encodeVarint_length_le
-- payload layer
#print axioms decode_encode
#print axioms enc_injective
#print axioms simpleSend_truncated
#print axioms activateFeature_truncated
#print axioms contractDexTrade_truncated
#print axioms sendMany_head_truncated
#print axioms sendMany_prefix_is_shorter_list
#print axioms simpleSend_trailing
-- activation
#print axioms unknown_ids_have_no_slot
#print axioms aliased_features
#print axioms isActive_iff
#print axioms not_active_before
#print axioms unknown_never_active
#print axioms active_mono
#print axioms activate_window
#print axioms activate_not_live
#print axioms activate_unknown
#print axioms activate_sets_slot
#print axioms activate_then_active
#print axioms deactivate_requires_live
#print axioms deactivate_inactive
#print axioms contractdex_deactivation_is_ineffective
#print axioms allowed_mono
#print axioms allowed_exact_version
#print axioms activation_version1_never_allowed
