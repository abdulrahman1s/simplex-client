//! Tests of the double ratchet (src/ratchet): ported from simplexmq's RatchetTests.hs, plus known answers from an independent TypeScript implementation and the reference C KEM.

mod kem_vectors;
mod legacy;
mod limits;
mod order;
mod params;
mod persist;
mod pq_switch;
mod resync;
mod sizes;
mod support;
mod ts_reference;
mod two_phase;
mod version;
mod x3dh;
