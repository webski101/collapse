# Windows next steps (after API key is saved)

You already saved the key to:

`C:\Users\USER\.config\collapse\atlas_key`

Do **not** paste the key in chat.

## 1. Open the project

In Cursor: **File → Open Folder** and open the folder that contains:

- `Cargo.toml`
- `collapse\`
- `scripts\`

In PowerShell, check:

```powershell
cd <path-to-that-folder>
dir
```

You must see `Cargo.toml`.

## 2. Install Rust (once)

```powershell
rustc --version
```

If missing: install from https://rustup.rs (choose the default MSVC toolchain).  
Also install **Visual Studio Build Tools** with “Desktop development with C++” if `cargo build` asks for a linker.

## 3. Test the API key

```powershell
.\scripts\test_atlas.ps1
```

Success looks like: `OK · mode=live · engine=retrocausal-echo-v1`

## 4. Build the Windows VST3

```powershell
.\scripts\bundle_vst3.ps1
```

Copy `dist\COLLAPSE.vst3` into:

`C:\Program Files\Common Files\VST3\`

Rescan plugins in Reaper / Ableton / FL, load **COLLAPSE**, press the button, record demos.

## 5. Submit

Form: https://airtable.com/appsrkUE9iVgeGsH5/pagdAHP56ovMdYX7x/form  
Deadline: **5 Oct (PT)** per hack.mothquantum.com
