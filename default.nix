# helix-improved: our helix fork with the shared toolkit (tuikit, a git dependency).
#
# The unwrapped editor is built here. The runtime directory (queries plus the
# tree-sitter grammars built by Nix) comes from nixpkgs' own `helix` wrapper,
# which we reuse by swapping in our unwrapped build and our own `grammars.json`
# (revisions from the fork's languages.toml, so they match its queries; see
# FORK.md for how to regenerate it).
{
    lib,
    craneLib,
    runCommand,
    helix,
    helix-unwrapped,
    src,
    # A tuikit checkout to use instead of the git dependency (null: the locked git revision).
    tuikitSrc ? null,
}:
let
    # `src` is the repo; tuikit comes in as a git dependency of helix-term (locked in Cargo.lock).
    helixSrc = src;

    # What helix falls back to when HELIX_RUNTIME is unset: themes, languages
    # and so on, without the queries and grammars that the wrapper supplies.
    defaultRuntimeDir = runCommand "helix-fork-default-runtime" { } ''
        cp -r --no-preserve=mode ${helixSrc}/runtime $out
        rm -rf $out/grammars $out/queries
    '';

    tuikitCrates = ["diagnostics-panel" "files" "git-core" "git-panel" "pick" "pv" "symbols-panel" "tb" "ui"];

    commonArgs = {
        pname = "helix-unwrapped";
        inherit (helix-unwrapped) version;
        inherit src;
        cargoLock = ./Cargo.lock;
        strictDeps = true;

        # Only the editor binary. The workspace's own tests are upstream's and
        # slow; `pick`, `pv`, `git-core` and `ui` are tested in tuikit.
        cargoExtraArgs = "--package helix-term";
        doCheck = false;

        env = {
            # Grammars are built by Nix, not by helix-term's build.rs.
            HELIX_DISABLE_AUTO_GRAMMAR_BUILD = "1";
            # A faster-to-build release: no LTO and parallel codegen cost a few
            # percent of runtime speed. Part of the shared args, so the
            # dependency build below stays valid for it.
            CARGO_PROFILE_RELEASE_LTO = "off";
            CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16";
            CARGO_PROFILE_RELEASE_DEBUG = "0";
        };
    };

    # Points the tuikit git dependency at the given checkout with a cargo `[patch]`. It is added
    # to the unpacked source, so the source stays a plain path (no build needed to evaluate).
    # The lockfile still names the git revision, so the build runs without `--locked` and cargo
    # re-resolves the patched crates. Only the editor build gets it, not `commonArgs`: the
    # dependency build below must not change when tuikit does, or every dependency (tree-sitter,
    # gix...) would compile again on each tuikit edit. It builds the locked tuikit revision, and
    # the editor build recompiles just the patched tuikit crates and helix-term.
    postPatch = lib.optionalString (tuikitSrc != null) ''
        mkdir -p .cargo
        cat >> .cargo/config.toml <<'EOF'

        [patch."https://github.com/chetansinghsajwan/tuikit"]
        ${lib.concatMapStringsSep "\n" (c: "tuikit-${c} = { path = \"${tuikitSrc}/crates/${c}\" }") tuikitCrates}
        EOF
    '';

    # All dependencies, built once from the manifests alone: editing our own
    # code (helix, crates) does not rebuild them.
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;

    unwrapped = craneLib.buildPackage (
        commonArgs
        // {
            inherit cargoArtifacts postPatch;
            env = commonArgs.env // {
                HELIX_DEFAULT_RUNTIME = defaultRuntimeDir;
            };
            meta = helix-unwrapped.meta // {
                description = "Helix with the shared pick picker";
            };
        }
    );
in
# The wrapper reads `helix-unwrapped.src` for runtime/queries.
helix.override {
    helix-unwrapped = unwrapped // {
        src = helixSrc;
    };
    lockedGrammars = lib.importJSON ./grammars.json;
}
