# FDArrayTest257 CID fixture provenance

Stored fixture: `ot-cff-cid-keyed.otf`

- Upstream repository: <https://github.com/adobe-fonts/fdarray-test>
- Upstream file: `FDArrayTest257.otf`
- Upstream commit: `e0b4382dee1625833b5f9b214eac0676d8ec7334`
- License: SIL Open Font License 1.1, copied in `FDArrayTest257.LICENSE.txt`
- SHA-256: `211f9ecb8b8064931f860e84bfe6e746e926273ef924990887ff2df13e6fede7`

Why this fixture is used:

- The upstream README describes `FDArrayTest257.otf` as a special-purpose
  CID-keyed OpenType/CFF font based on Adobe-Identity-0 ROS.
- The fixture is small enough for the repo and exercises the SFNT-wrapped CID
  service path needed by `FT_Get_CID_From_Glyph_Index` and
  `FT_Get_CID_Is_Internally_CID_Keyed`.
- Non-SFNT Type 1 CID rows remain pending; this fixture must not be used to
  satisfy those separate `cid_keyed_font` cases.
