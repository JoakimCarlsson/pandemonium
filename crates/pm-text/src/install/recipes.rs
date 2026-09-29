//! Pinned recipes for every server the editor can install.

use super::{Build, Recipe};

/// The built-in installation recipe for `command`, when one exists.
pub const fn recipe(command: &str) -> Option<Recipe> {
    if same(command, "rust-analyzer") {
        return Some(Recipe::Release {
            version: "2026-09-28",
            builds: &[
                Build {
                    platform: "linux-x86_64",
                    url: "https://github.com/rust-lang/rust-analyzer/releases/download/{version}/rust-analyzer-x86_64-unknown-linux-gnu.gz",
                    sha256: "23f711d86b5f826e22886f01d7355dc01e0f4c1357dafa29710a95b903b48c85",
                },
                Build {
                    platform: "linux-aarch64",
                    url: "https://github.com/rust-lang/rust-analyzer/releases/download/{version}/rust-analyzer-aarch64-unknown-linux-gnu.gz",
                    sha256: "03bad9c3dabb0f07a2678d5f9f8f1575a3742ea141506e14b3a26b42a1f896f3",
                },
                Build {
                    platform: "macos-x86_64",
                    url: "https://github.com/rust-lang/rust-analyzer/releases/download/{version}/rust-analyzer-x86_64-apple-darwin.gz",
                    sha256: "d032c0eb75e4597cc8ffc35ea4cdbd9eecc8341936b6edac6749e679fc3f0682",
                },
                Build {
                    platform: "macos-aarch64",
                    url: "https://github.com/rust-lang/rust-analyzer/releases/download/{version}/rust-analyzer-aarch64-apple-darwin.gz",
                    sha256: "54ec873d8996e2c127d758bf45d4eacb6d3371dae4f6f6d5d3f05cedbae5fd59",
                },
                Build {
                    platform: "windows-x86_64",
                    url: "https://github.com/rust-lang/rust-analyzer/releases/download/{version}/rust-analyzer-x86_64-pc-windows-msvc.zip",
                    sha256: "ad78fb368525404c6ac09c4bba33e90797902ce1f5a17db0925c695cae096ccc",
                },
                Build {
                    platform: "windows-aarch64",
                    url: "https://github.com/rust-lang/rust-analyzer/releases/download/{version}/rust-analyzer-aarch64-pc-windows-msvc.zip",
                    sha256: "f63c7fc9a00a7e863b21b5e0b77cda7ff61aaa9f6f83b0affa5bbb525be7c43c",
                },
            ],
        });
    }
    if same(command, "taplo") {
        return Some(Recipe::Release {
            version: "0.10.0",
            builds: &[
                Build {
                    platform: "linux-x86_64",
                    url: "https://github.com/tamasfe/taplo/releases/download/{version}/taplo-linux-x86_64.gz",
                    sha256: "8fe196b894ccf9072f98d4e1013a180306e17d244830b03986ee5e8eabeb6156",
                },
                Build {
                    platform: "linux-aarch64",
                    url: "https://github.com/tamasfe/taplo/releases/download/{version}/taplo-linux-aarch64.gz",
                    sha256: "033681d01eec8376c3fd38fa3703c79316f5e14bb013d859943b60a07bccdcc3",
                },
                Build {
                    platform: "linux-riscv64",
                    url: "https://github.com/tamasfe/taplo/releases/download/{version}/taplo-linux-riscv64.gz",
                    sha256: "26b2d848183636f291f7e2ea4e5dfd3622ff5f445417333d33a32082fc582914",
                },
                Build {
                    platform: "macos-x86_64",
                    url: "https://github.com/tamasfe/taplo/releases/download/{version}/taplo-darwin-x86_64.gz",
                    sha256: "898122cde3a0b1cd1cbc2d52d3624f23338218c91b5ddb71518236a4c2c10ef2",
                },
                Build {
                    platform: "macos-aarch64",
                    url: "https://github.com/tamasfe/taplo/releases/download/{version}/taplo-darwin-aarch64.gz",
                    sha256: "713734314c3e71894b9e77513c5349835eefbd52908445a0d73b0c7dc469347d",
                },
                Build {
                    platform: "windows-x86_64",
                    url: "https://github.com/tamasfe/taplo/releases/download/{version}/taplo-windows-x86_64.zip",
                    sha256: "1615eed140039bd58e7089109883b1c434de5d6de8f64a993e6e8c80ca57bdf9",
                },
                Build {
                    platform: "windows-aarch64",
                    url: "https://github.com/tamasfe/taplo/releases/download/{version}/taplo-windows-aarch64.zip",
                    sha256: "65a50c5d3b78f6014e6bc6d64eb6dc1d4992bc236589c9bb29e5609fc3454674",
                },
            ],
        });
    }
    if same(command, "marksman") {
        return Some(Recipe::Release {
            version: "2026-02-08",
            builds: &[
                Build {
                    platform: "linux-x86_64",
                    url: "https://github.com/artempyanykh/marksman/releases/download/{version}/marksman-linux-x64",
                    sha256: "be5098e8213219269c47fc0d916a66fa31ce0602ec967475c722260aabf26087",
                },
                Build {
                    platform: "linux-aarch64",
                    url: "https://github.com/artempyanykh/marksman/releases/download/{version}/marksman-linux-arm64",
                    sha256: "db8e124527f7f8048e3e6c91821b9c52ef173d92c01e47d221bf1337afd962fb",
                },
                Build {
                    platform: "macos-x86_64",
                    url: "https://github.com/artempyanykh/marksman/releases/download/{version}/marksman-macos",
                    sha256: "6a801c17b5ac0dba69787c5282b3b3bd416e66c96253fae098d311c6bbd1833b",
                },
                Build {
                    platform: "macos-aarch64",
                    url: "https://github.com/artempyanykh/marksman/releases/download/{version}/marksman-macos",
                    sha256: "6a801c17b5ac0dba69787c5282b3b3bd416e66c96253fae098d311c6bbd1833b",
                },
                Build {
                    platform: "windows-x86_64",
                    url: "https://github.com/artempyanykh/marksman/releases/download/{version}/marksman.exe",
                    sha256: "a6d05beb08ebe41b0a9f09c98a438540421436fa5531424c22e0bb1d22529705",
                },
                Build {
                    platform: "windows-aarch64",
                    url: "https://github.com/artempyanykh/marksman/releases/download/{version}/marksman.exe",
                    sha256: "a6d05beb08ebe41b0a9f09c98a438540421436fa5531424c22e0bb1d22529705",
                },
            ],
        });
    }
    if same(command, "lua-language-server") {
        return Some(Recipe::Release {
            version: "3.19.1",
            builds: &[
                Build {
                    platform: "linux-x86_64",
                    url: "https://github.com/LuaLS/lua-language-server/releases/download/{version}/lua-language-server-3.19.1-linux-x64.tar.gz",
                    sha256: "e9235d2d72ef55bc41cf8c99cda2ed64777682024b4bb81f5dea425060c5cbb8",
                },
                Build {
                    platform: "linux-aarch64",
                    url: "https://github.com/LuaLS/lua-language-server/releases/download/{version}/lua-language-server-3.19.1-linux-arm64.tar.gz",
                    sha256: "abd2572e8fc929dc838a81ffb8473c5bce0bf39bfe8edb4b120b3b623176ce83",
                },
                Build {
                    platform: "macos-x86_64",
                    url: "https://github.com/LuaLS/lua-language-server/releases/download/{version}/lua-language-server-3.19.1-darwin-x64.tar.gz",
                    sha256: "eb373c159cbe556711d7cd316315de2dce969bfd54b31edb7eb9cab2937f2cca",
                },
                Build {
                    platform: "macos-aarch64",
                    url: "https://github.com/LuaLS/lua-language-server/releases/download/{version}/lua-language-server-3.19.1-darwin-arm64.tar.gz",
                    sha256: "0bc077f4447f076b4c92c14e9fd303f5b569eda2ec74b4dca2b55f75fae2e90c",
                },
                Build {
                    platform: "windows-x86_64",
                    url: "https://github.com/LuaLS/lua-language-server/releases/download/{version}/lua-language-server-3.19.1-win32-x64.zip",
                    sha256: "fdb9a59108cf62517813c97fa5549b0e16d1ef0688306bac728b08434db7e4cd",
                },
            ],
        });
    }
    if same(command, "clangd") {
        return Some(Recipe::Release {
            version: "22.1.6",
            builds: &[
                Build {
                    platform: "linux-x86_64",
                    url: "https://github.com/clangd/clangd/releases/download/{version}/clangd-linux-22.1.6.zip",
                    sha256: "a9c77443af2e447ed467e84771848d3a6ac1c56f84bcfcde717e66318de77cfa",
                },
                Build {
                    platform: "macos-x86_64",
                    url: "https://github.com/clangd/clangd/releases/download/{version}/clangd-mac-22.1.6.zip",
                    sha256: "631aef462556cbd74e0ebaae1778a38d1997d0ba3371652ca54f82652a179e7d",
                },
                Build {
                    platform: "macos-aarch64",
                    url: "https://github.com/clangd/clangd/releases/download/{version}/clangd-mac-22.1.6.zip",
                    sha256: "631aef462556cbd74e0ebaae1778a38d1997d0ba3371652ca54f82652a179e7d",
                },
                Build {
                    platform: "windows-x86_64",
                    url: "https://github.com/clangd/clangd/releases/download/{version}/clangd-windows-22.1.6.zip",
                    sha256: "ce54f16e0b4fd76d450eeda9664420b195360b73febcfe40e661108fa57f2ce1",
                },
            ],
        });
    }
    if same(command, "ruff") {
        return Some(Recipe::Release {
            version: "0.16.9",
            builds: &[
                Build {
                    platform: "linux-x86_64",
                    url: "https://github.com/astral-sh/ruff/releases/download/{version}/ruff-x86_64-unknown-linux-gnu.tar.gz",
                    sha256: "1bfbb819b5d4f9af501748862276b60e412d336034d99387691a4d4bce7a6f13",
                },
                Build {
                    platform: "linux-aarch64",
                    url: "https://github.com/astral-sh/ruff/releases/download/{version}/ruff-aarch64-unknown-linux-gnu.tar.gz",
                    sha256: "a13061e8f471b49c9d2aa284c32dd54a0e5534d702d1a44e1d7f4c875569586d",
                },
                Build {
                    platform: "macos-x86_64",
                    url: "https://github.com/astral-sh/ruff/releases/download/{version}/ruff-x86_64-apple-darwin.tar.gz",
                    sha256: "e98ea259a021c87d3a1f8bf18639d2e32dcd45cb2ef1afcb41b13e295de1e2b3",
                },
                Build {
                    platform: "macos-aarch64",
                    url: "https://github.com/astral-sh/ruff/releases/download/{version}/ruff-aarch64-apple-darwin.tar.gz",
                    sha256: "33d35394499094cf6eb90f730dc82f11c0fab05d176378ae4f67de985ecc4146",
                },
                Build {
                    platform: "windows-x86_64",
                    url: "https://github.com/astral-sh/ruff/releases/download/{version}/ruff-x86_64-pc-windows-msvc.zip",
                    sha256: "fe5eb06b2f185b0d8035e17df9df0c831e59b3c98f11170206500650218a0b76",
                },
                Build {
                    platform: "windows-aarch64",
                    url: "https://github.com/astral-sh/ruff/releases/download/{version}/ruff-aarch64-pc-windows-msvc.zip",
                    sha256: "98f41e001af9ccbbd16a9948b1214e711af9bacfaa204951d831584dfe109fcf",
                },
            ],
        });
    }
    if same(command, "biome") {
        return Some(Recipe::Release {
            version: "2.5.14",
            builds: &[
                Build {
                    platform: "linux-x86_64",
                    url: "https://github.com/biomejs/biome/releases/download/%40biomejs/biome%40{version}/biome-linux-x64",
                    sha256: "290c1c85deeaf01310d9187306060b53961574856a7e34f59d9662562e6f19fe",
                },
                Build {
                    platform: "linux-aarch64",
                    url: "https://github.com/biomejs/biome/releases/download/%40biomejs/biome%40{version}/biome-linux-arm64",
                    sha256: "50ac7f598985e21b15da57e5d87da0f7ab0c163e6f78eb6d4690309827d77ef8",
                },
                Build {
                    platform: "macos-x86_64",
                    url: "https://github.com/biomejs/biome/releases/download/%40biomejs/biome%40{version}/biome-darwin-x64",
                    sha256: "b331448d7afb592cc4674e9bd6905eae5e79f5dd9b34b61b44803a6b7a91b801",
                },
                Build {
                    platform: "macos-aarch64",
                    url: "https://github.com/biomejs/biome/releases/download/%40biomejs/biome%40{version}/biome-darwin-arm64",
                    sha256: "3d1194d0a7b720315fb6f8cbafeb5b18ca7100c3e635e8ce82ef46b315de9c0e",
                },
                Build {
                    platform: "windows-x86_64",
                    url: "https://github.com/biomejs/biome/releases/download/%40biomejs/biome%40{version}/biome-win32-x64.exe",
                    sha256: "bef8f088617c8364314f55dffdc7b8ddabe12fee9e1e71f45f708a7027579280",
                },
                Build {
                    platform: "windows-aarch64",
                    url: "https://github.com/biomejs/biome/releases/download/%40biomejs/biome%40{version}/biome-win32-arm64.exe",
                    sha256: "af61f09b037a6cdf24ed3b37664de83e9b61bf8a4c8dd691ea6b64e808c035c8",
                },
            ],
        });
    }
    if same(command, "typescript-language-server") {
        return Some(Recipe::Npm {
            package: "typescript-language-server",
            version: "6.0.1",
            extra: &["typescript@7.0.2"],
        });
    }
    if same(command, "basedpyright-langserver") {
        return Some(Recipe::Npm {
            package: "basedpyright",
            version: "1.40.1",
            extra: &[],
        });
    }
    if same(command, "bash-language-server") {
        return Some(Recipe::Npm {
            package: "bash-language-server",
            version: "5.8.1",
            extra: &[],
        });
    }
    if same(command, "yaml-language-server") {
        return Some(Recipe::Npm {
            package: "yaml-language-server",
            version: "1.24.0",
            extra: &[],
        });
    }
    if same(command, "intelephense") {
        return Some(Recipe::Npm {
            package: "intelephense",
            version: "1.18.5",
            extra: &[],
        });
    }
    if same(command, "tailwindcss-language-server") {
        return Some(Recipe::Npm {
            package: "@tailwindcss/language-server",
            version: "0.16.0",
            extra: &[],
        });
    }
    if same(command, "docker-langserver") {
        return Some(Recipe::Npm {
            package: "dockerfile-language-server-nodejs",
            version: "0.15.0",
            extra: &[],
        });
    }
    if same(command, "vscode-json-language-server")
        || same(command, "vscode-css-language-server")
        || same(command, "vscode-html-language-server")
        || same(command, "vscode-eslint-language-server")
    {
        return Some(Recipe::Npm {
            package: "vscode-langservers-extracted",
            version: "4.10.0",
            extra: &[],
        });
    }
    if same(command, "tsc") {
        return Some(Recipe::Npm {
            package: "typescript",
            version: "7.0.2",
            extra: &[],
        });
    }
    if same(command, "vtsls") {
        return Some(Recipe::Npm {
            package: "@vtsls/language-server",
            version: "0.3.0",
            extra: &[],
        });
    }
    if same(command, "pyright-langserver") {
        return Some(Recipe::Npm {
            package: "pyright",
            version: "1.1.414",
            extra: &[],
        });
    }
    if same(command, "gopls") {
        return Some(Recipe::Go {
            module: "golang.org/x/tools/gopls",
            version: "v0.23.0",
        });
    }
    if same(command, "sqls") {
        return Some(Recipe::Go {
            module: "github.com/sqls-server/sqls",
            version: "v0.2.48",
        });
    }
    if same(command, "pylsp") {
        return Some(Recipe::Pip {
            package: "python-lsp-server",
            version: "1.15.0",
        });
    }
    None
}

/// What `command` needs that the editor cannot install for it, and how to
/// get it, for a server with no recipe because it runs on a runtime of its own.
pub const fn needs(command: &str) -> Option<&'static str> {
    if same(command, "jdtls") {
        return Some("jdtls needs Java 21 or newer; install a JDK and jdtls yourself.");
    }
    if same(command, "kotlin-lsp") || same(command, "kotlin-language-server") {
        return Some("The Kotlin server needs Java; install a JDK and kotlin-lsp yourself.");
    }
    if same(command, "csharp-ls") {
        return Some(
            "csharp-ls needs .NET; install the .NET SDK, then run dotnet tool install -g csharp-ls.",
        );
    }
    if same(command, "OmniSharp") {
        return Some("OmniSharp needs .NET; install the .NET SDK and OmniSharp yourself.");
    }
    if same(command, "ruby-lsp") {
        return Some("ruby-lsp needs Ruby; install Ruby, then run gem install ruby-lsp.");
    }
    None
}

/// Whether two command names match in a constant recipe definition.
const fn same(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}
