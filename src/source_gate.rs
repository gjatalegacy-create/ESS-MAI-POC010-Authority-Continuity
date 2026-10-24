//! Minimal source extract of the v189 process-local one-use capability gate.
//!
//! Retained source blocks are byte-identical to provenance/source_gate.json.
//! The binding wrapper and tests below are new POC010 instrumentation. This module
//! has no native verdict, knowledge write, durable claim, C export, global registry,
//! or transferable authority.
//!
//! A gate belongs to one fixture-server lifetime and is guarded by its owner.
//! Generations restart with each gate; handles must not be persisted, restored,
//! or interpreted in another gate. Nonces are deterministic module labels, not
//! secrets, authentication, or semantic action identities.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

// ════════════════════════════════════════════════════════════════════════════
// §0  MATEMATIKA BAZË — FNV-1a (e njëjtë me projektin)
// ════════════════════════════════════════════════════════════════════════════

const fn fnv(s: &[u8]) -> u64 {
    let mut h: u64 = 14695981039346656037;
    let mut i = 0;
    while i < s.len() {
        h ^= s[i] as u64;
        h = h.wrapping_mul(1099511628211);
        i += 1;
    }
    h
}
const fn seal(m: &[u8]) -> u64 { fnv(m) ^ fnv(b"GJATA_LEGACY") }

// Compile-time nonce-ët — të prera, jo të negociueshme
pub const S_SHADOW_WRITE:  u64 = seal(b"SHADOW_WRITE_KERNEL");

// ════════════════════════════════════════════════════════════════════════════
// §1  LGC TOKEN — PATHYESHËM, KURRË nuk kalon FFI
//     PhantomData<*const ()> → auto: !Send !Clone !Copy
// ════════════════════════════════════════════════════════════════════════════

pub struct LgcToken(PhantomData<*const ()>);

// ════════════════════════════════════════════════════════════════════════════
// §2  CAP HANDLE — E VETMJA gjë që C sheh dhe prek
//     repr(C) → C e lexon si dy u64. Copy → C bën memcpy lirisht (KURTHI).
// ════════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CapHandle {
    pub gen:   u64, // ID gjenerimi — identifikon CapSlot në Rust heap
    pub nonce: u64, // FNV seal e modulit — verifikim matematik
}

// ════════════════════════════════════════════════════════════════════════════
// §3  CAP SLOT — ZEMRA e garancisë; KURRË nuk del nga Rust
//     AtomicBool: true→i freskët (1× konsum me CAS), false→i djegur (kurrë rikthim)
// ════════════════════════════════════════════════════════════════════════════

struct CapSlot {
    valid:          AtomicBool, // burned me CAS — JO me store()
    gen:            u64,
    expected_nonce: u64,
}

#[derive(Debug, PartialEq)]
pub enum FfiErr {
    HandleNotFound,
    NonceMismatch,
    AlreadyConsumed, // ← KURTHI I SULMIT C: CAS i rënë në burned slot
}

// ════════════════════════════════════════════════════════════════════════════
// §4  SOVEREIGN GATE — Registry i fshehur: HashMap<gen → Box<CapSlot>>
//     C ka gen (numri). C nuk ka *Box<CapSlot> (adresën e realitetit).
// ════════════════════════════════════════════════════════════════════════════

struct SovereignGate {
    slots:   HashMap<u64, Box<CapSlot>>,
    counter: u64,
}

impl SovereignGate {
    fn new() -> Self { Self { slots: HashMap::new(), counter: 1 } }

    /// Krijon CapSlot në heap Rust, kthen vetëm CapHandle (dy numra) tek C.
    fn issue(&mut self, module_nonce: u64) -> CapHandle {
        let gen = self.counter;
        self.counter += 1;
        self.slots.insert(gen, Box::new(CapSlot {
            valid: AtomicBool::new(true),
            gen,
            expected_nonce: module_nonce,
        }));
        CapHandle { gen, nonce: module_nonce }
    }

    // ═══ MURI MATEMATIK ═══════════════════════════════════════════════════
    //   compare_exchange(true→false, AcqRel, Acquire)
    //     valid==true  → atomikisht false → Ok  → LgcToken (1× konsum)
    //     valid==false → s'ndryshon       → Err → AlreadyConsumed (burned)
    fn validate_and_burn(&self, h: CapHandle) -> Result<LgcToken, FfiErr> {
        let slot = self.slots.get(&h.gen).ok_or(FfiErr::HandleNotFound)?;
        // Gjenerata në handle duhet të jetë po ajo që mban slot-i real.
        match slot.gen != h.gen { true => return Err(FfiErr::HandleNotFound), false => {} }
        // LAW_5: NK-check (nonce matematik) PARA operacionit.
        match slot.expected_nonce != h.nonce { true => return Err(FfiErr::NonceMismatch), false => {} }
        slot.valid
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| LgcToken(PhantomData))
            .map_err(|_| FfiErr::AlreadyConsumed)
    }
}

// NEW POC010 glue: exact action-and-material binding before the source burn.
// This binding map does not deduplicate fresh authorizations for the same action.
pub(crate) struct BoundGate {
    core: SovereignGate,
    bindings: HashMap<u64, Vec<u8>>,
}

impl BoundGate {
    pub(crate) fn new() -> Self {
        Self {
            core: SovereignGate::new(),
            bindings: HashMap::new(),
        }
    }

    pub(crate) fn issue(&mut self, binding: &[u8]) -> CapHandle {
        let handle = self.core.issue(S_SHADOW_WRITE);
        self.bindings.insert(handle.gen, binding.to_vec());
        handle
    }

    pub(crate) fn consume(&self, handle: CapHandle, binding: &[u8]) -> Result<(), String> {
        let expected = self
            .bindings
            .get(&handle.gen)
            .ok_or_else(|| "HandleNotFound".to_string())?;
        if expected.as_slice() != binding {
            return Err("BindingMismatch".to_string());
        }
        // The source token is consumed locally by this closure and never escapes.
        self.core
            .validate_and_burn(handle)
            .map(|_token| ())
            .map_err(|error| format!("{error:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundGate, CapHandle};

    #[test]
    fn copied_handle_does_not_copy_one_use_authority() {
        let binding = b"canonical action and material";
        let mut gate = BoundGate::new();
        let handle = gate.issue(binding);
        let copied = handle;
        assert_eq!(gate.consume(handle, binding), Ok(()));
        assert_eq!(gate.consume(copied, binding), Err("AlreadyConsumed".into()));
        assert_eq!(gate.consume(handle, binding), Err("AlreadyConsumed".into()));
    }

    #[test]
    fn fresh_handles_for_same_binding_both_pass_baseline() {
        let binding = b"same canonical action and material";
        let mut gate = BoundGate::new();
        let first = gate.issue(binding);
        let second = gate.issue(binding);
        assert_ne!(first.gen, second.gen);
        assert_eq!(first.nonce, second.nonce);
        assert_eq!(gate.consume(first, binding), Ok(()));
        assert_eq!(gate.consume(second, binding), Ok(()));
    }

    #[test]
    fn wrong_binding_does_not_burn_the_valid_handle() {
        let binding = b"bound action and material";
        let mut gate = BoundGate::new();
        let handle = gate.issue(binding);
        assert_eq!(gate.consume(handle, b"different action or material"), Err("BindingMismatch".into()));
        assert_eq!(gate.consume(handle, binding), Ok(()));
    }

    #[test]
    fn wrong_nonce_and_unknown_handle_fail_before_burn() {
        let binding = b"bound action and material";
        let mut gate = BoundGate::new();
        let handle = gate.issue(binding);
        let wrong_nonce = CapHandle { gen: handle.gen, nonce: handle.nonce ^ 1 };
        let unknown = CapHandle { gen: handle.gen + 100, nonce: handle.nonce };
        assert_eq!(gate.consume(wrong_nonce, binding), Err("NonceMismatch".into()));
        assert_eq!(gate.consume(unknown, binding), Err("HandleNotFound".into()));
        assert_eq!(gate.consume(handle, binding), Ok(()));
    }
}
