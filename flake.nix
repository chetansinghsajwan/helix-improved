# helix-improved: our helix fork, built with crane. The shared toolkit (tuikit) is a git dependency
# of helix-term; a build that has its own tuikit checkout (dotfiles) patches it in, see
# pkgs/flake.nix there.
{
    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

        # Splits cargo builds into a cached dependency build and our code.
        crane.url = "github:ipetkov/crane";
    };

    outputs =
        { nixpkgs, crane, ... }:
        let
            forEachSystem = nixpkgs.lib.genAttrs [
                "x86_64-linux"
                "aarch64-linux"
                "x86_64-darwin"
                "aarch64-darwin"
            ];

            # The part of this repo the build needs, so editing anything else does not rebuild it.
            # Never local build output (target/) or the fetched grammar sources.
            sourceOf =
                lib:
                lib.cleanSourceWith {
                    src = ./.;
                    filter =
                        path: _:
                        let
                            rel = lib.removePrefix "${toString ./.}/" (toString path);
                        in
                        (
                            rel == (toString ./.)
                            || lib.any (d: rel == d || lib.hasPrefix "${d}/" rel) [
                                ".cargo"
                                "helix-core"
                                "helix-dap"
                                "helix-dap-types"
                                "helix-event"
                                "helix-loader"
                                "helix-lsp"
                                "helix-lsp-types"
                                "helix-parsec"
                                "helix-stdx"
                                "helix-term"
                                "helix-tui"
                                "helix-vcs"
                                "helix-view"
                                "runtime"
                                "xtask"
                            ]
                            || builtins.elem rel [
                                "Cargo.toml"
                                "Cargo.lock"
                                "rust-toolchain.toml"
                                "rustfmt.toml"
                                "languages.toml"
                                "theme.toml"
                                "base16_theme.toml"
                            ]
                        )
                        && baseNameOf path != "target"
                        && !(lib.hasSuffix "runtime/grammars/sources" rel);
                };

            # The editor from a given source tree, so a parent repo can build it from its own copy
            # (dotfiles patches in its local tuikit).
            helixFrom =
                pkgs: src:
                pkgs.callPackage ./default.nix {
                    craneLib = crane.mkLib pkgs;
                    inherit src;
                };
        in
        {
            lib = {
                inherit sourceOf helixFrom;
            };

            packages = forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                    helix = helixFrom pkgs (sourceOf pkgs.lib);
                in
                {
                    inherit helix;
                    default = helix;
                }
            );
        };
}
