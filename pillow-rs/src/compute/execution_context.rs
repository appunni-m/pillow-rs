//! Immutable backend and CPU architecture selection for image execution.

use super::Backend;
use std::sync::OnceLock;

/// CPU instruction set selected for one routed operation or pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    /// Portable scalar implementation.
    Default,
    /// AArch64 NEON implementation.
    Neon,
    /// x86 SSE2 implementation.
    Sse2,
    /// x86 SSE3 implementation.
    Sse3,
    /// x86 SSSE3 implementation.
    Ssse3,
    /// x86 AVX implementation.
    Avx,
    /// x86 AVX2 implementation.
    Avx2,
    /// x86 AVX implementation with FMA3 available.
    AvxFma,
    /// x86 AVX2 implementation with FMA3 available.
    Avx2Fma,
    /// WebAssembly's statically enabled 128-bit SIMD implementation.
    WasmSimd128,
}

impl Architecture {
    /// Returns the architecture policy selected once for this process.
    ///
    /// Portable builds detect host capabilities at this boundary. Leaf
    /// implementations receive the resulting value and never probe again.
    #[must_use]
    pub fn detected_host() -> Self {
        CpuCapabilities::host().preferred()
    }

    /// Returns the directory name for the architecture-specific implementation.
    #[must_use]
    pub const fn folder(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Neon => "neon",
            Self::Sse2 => "sse2",
            Self::Sse3 => "sse3",
            Self::Ssse3 => "ssse3",
            Self::Avx | Self::AvxFma => "avx",
            Self::Avx2 | Self::Avx2Fma => "avx2",
            Self::WasmSimd128 => "wasm_simd128",
        }
    }

    /// Whether this selected architecture includes FMA3.
    #[must_use]
    pub const fn has_fma(self) -> bool {
        matches!(self, Self::AvxFma | Self::Avx2Fma)
    }

}

/// Independently detected capabilities, kept at the context boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CpuCapabilities {
    neon: bool,
    sse2: bool,
    sse3: bool,
    ssse3: bool,
    avx: bool,
    avx2: bool,
    fma: bool,
    wasm_simd128: bool,
}

impl CpuCapabilities {
    const PORTABLE: Self = Self {
        neon: false,
        sse2: false,
        sse3: false,
        ssse3: false,
        avx: false,
        avx2: false,
        fma: false,
        wasm_simd128: false,
    };

    fn host() -> Self {
        static HOST: OnceLock<CpuCapabilities> = OnceLock::new();
        *HOST.get_or_init(Self::detect)
    }

    const fn supports(self, architecture: Architecture) -> bool {
        match architecture {
            Architecture::Default => true,
            Architecture::Neon => self.neon,
            Architecture::Sse2 => self.sse2,
            Architecture::Sse3 => self.sse3,
            Architecture::Ssse3 => self.ssse3,
            Architecture::Avx => self.avx,
            Architecture::Avx2 => self.avx2,
            Architecture::AvxFma => self.avx && self.fma,
            Architecture::Avx2Fma => self.avx2 && self.fma,
            Architecture::WasmSimd128 => self.wasm_simd128,
        }
    }

    fn preferred(self) -> Architecture {
        if self.avx2 {
            if self.fma { Architecture::Avx2Fma } else { Architecture::Avx2 }
        } else if self.avx {
            if self.fma { Architecture::AvxFma } else { Architecture::Avx }
        } else if self.ssse3 {
            Architecture::Ssse3
        } else if self.sse3 {
            Architecture::Sse3
        } else if self.sse2 {
            Architecture::Sse2
        } else if self.neon {
            Architecture::Neon
        } else if self.wasm_simd128 {
            Architecture::WasmSimd128
        } else {
            Architecture::Default
        }
    }

    fn detect() -> Self {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            return Self {
                sse2: std::is_x86_feature_detected!("sse2"),
                sse3: std::is_x86_feature_detected!("sse3"),
                ssse3: std::is_x86_feature_detected!("ssse3"),
                avx: std::is_x86_feature_detected!("avx"),
                avx2: std::is_x86_feature_detected!("avx2"),
                fma: std::is_x86_feature_detected!("fma"),
                ..Self::PORTABLE
            };
        }
        #[cfg(target_arch = "aarch64")]
        {
            Self { neon: cfg!(target_feature = "neon"), ..Self::PORTABLE }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self { wasm_simd128: cfg!(target_feature = "simd128"), ..Self::PORTABLE }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64", target_arch = "wasm32")))]
        {
            Self::PORTABLE
        }
    }
}

/// Backend and CPU architecture selected at the pipeline or eager-operation boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionContext {
    backend: Backend,
    architecture: Architecture,
    capabilities: CpuCapabilities,
}

impl ExecutionContext {
    /// Resolves the existing public routing choice once at an execution boundary.
    ///
    /// The compatibility SIMD request denotes CPU execution with the centrally
    /// selected host architecture. Ordinary CPU remains explicitly serial Default.
    pub(crate) fn for_routed_backend(backend: Backend) -> Self {
        let (backend, architecture) = match backend {
            Backend::Cpu => (Backend::Cpu, Architecture::Default),
            Backend::Simd => (Backend::Cpu, Architecture::detected_host()),
            Backend::Gpu => (Backend::Gpu, Architecture::Default),
        };
        Self::new(backend, architecture)
    }

    /// Captures routing policy and the host capabilities detected once.
    ///
    /// Strategy admission checks [`Self::supports_architecture`] before optional
    /// instructions execute. An unavailable requested architecture is retained
    /// so the prepared plan can explain its explicit Default fallback.
    #[must_use]
    pub fn new(backend: Backend, architecture: Architecture) -> Self {
        Self {
            backend,
            architecture,
            capabilities: CpuCapabilities::host(),
        }
    }

    /// Returns the selected execution backend.
    #[must_use]
    pub const fn backend(self) -> Backend {
        self.backend
    }

    /// Returns the selected CPU architecture.
    #[must_use]
    pub const fn architecture(self) -> Architecture {
        self.architecture
    }

    /// Whether a concrete implementation belongs to the injected architecture policy.
    ///
    /// Required instruction sets are checked independently after this policy
    /// relation; AVX2 alone does not establish SSSE3 or FMA availability.
    #[must_use]
    pub const fn permits_architecture(self, implementation: Architecture) -> bool {
        match self.architecture {
            Architecture::Default => matches!(implementation, Architecture::Default),
            Architecture::Neon => matches!(implementation, Architecture::Default | Architecture::Neon),
            Architecture::WasmSimd128 => matches!(implementation,
                Architecture::Default | Architecture::WasmSimd128),
            Architecture::Sse2 => matches!(implementation, Architecture::Default | Architecture::Sse2),
            Architecture::Sse3 => matches!(implementation,
                Architecture::Default | Architecture::Sse2 | Architecture::Sse3),
            Architecture::Ssse3 => matches!(implementation,
                Architecture::Default | Architecture::Sse2 | Architecture::Sse3 | Architecture::Ssse3),
            Architecture::Avx => matches!(implementation,
                Architecture::Default | Architecture::Sse2 | Architecture::Sse3 | Architecture::Ssse3 | Architecture::Avx),
            Architecture::AvxFma => matches!(implementation,
                Architecture::Default | Architecture::Sse2 | Architecture::Sse3 | Architecture::Ssse3
                    | Architecture::Avx | Architecture::AvxFma),
            Architecture::Avx2 => matches!(implementation,
                Architecture::Default | Architecture::Sse2 | Architecture::Sse3 | Architecture::Ssse3
                    | Architecture::Avx | Architecture::Avx2),
            Architecture::Avx2Fma => matches!(implementation,
                Architecture::Default | Architecture::Sse2 | Architecture::Sse3 | Architecture::Ssse3
                    | Architecture::Avx | Architecture::AvxFma
                    | Architecture::Avx2 | Architecture::Avx2Fma),
        }
    }

    /// Whether the captured host supports the exact required instructions.
    #[must_use]
    pub const fn supports_architecture(self, architecture: Architecture) -> bool {
        self.capabilities.supports(architecture)
    }
}
