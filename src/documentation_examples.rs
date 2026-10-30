// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runs examples directly from the bilingual README files and user guides.

#[doc = include_str!("../README.md")]
mod readme_en {
    // empty
}

#[doc = include_str!("../README.zh_CN.md")]
mod readme_zh {
    // empty
}

#[doc = include_str!("../doc/user_guide.md")]
mod user_guide_en {
    // empty
}

#[doc = include_str!("../doc/user_guide.zh_CN.md")]
mod user_guide_zh {
    // empty
}
