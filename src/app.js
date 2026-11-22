/* CookieSincraft — frontend app (wired to real Tauri commands)
 *
 * load_image / upscale / cancel_job / export_image are invoked via
 * @tauri-apps/api. File selection uses the Tauri dialog API so the
 * backend receives real absolute paths (Tauri 2 does not expose paths
 * from <input type="file"> or drag-and-drop File objects).
 */
import { invoke, Channel } from "@tauri-apps/api/core"

const $ = (id) => document.getElementById(id)

// ---- Elements ----
const dropZone = $("dropZone")
const fileName = $("fileName")
const previewWrap = $("previewWrap")
const previewPlaceholder = $("previewPlaceholder")
const previewImg = $("previewImg")
const backendSelect = $("backend")
const algorithmSelect = $("algorithm")
const resolutionSelect = $("resolution")
const qualityInput = $("quality")
const qualityValue = $("qualityValue")
const btnStart = $("btnStart")
const btnCancel = $("btnCancel")
const btnExport = $("btnExport")
const progressFill = $("progressFill")
const progressPct = $("progressPct")
const statusText = $("statusText")
// Convert elements
const convertFormat = $("convertFormat")
const convertQuality = $("convertQuality")
const convertQualityValue = $("convertQualityValue")
const btnConvert = $("btnConvert")

// ---- Algorithm options by backend ----
const ALGORITHMS = {
  cpu: ["Nearest", "Bilinear", "CatmullRom", "Lanczos3"],
  gpu: ["FSR 1.0", "Anime4K", "NIS"],
}

// ---- State ----
let currentFile = null   // { path, name }
let currentJobId = null
let outputPath = null
let upscaleChannel = null

// ---- Utils ----
function setStatus(msg, kind = "muted") {
  statusText.textContent = msg
  statusText.className = "status-text"
  if (kind !== "muted") statusText.classList.add(kind)
}

function setProgress(pct) {
  const v = Math.max(0, Math.min(100, pct))
  progressFill.style.width = v + "%"
  progressPct.textContent = v + "%"
}

function resetJob() {
  currentJobId = null
  outputPath = null
  upscaleChannel = null
  setProgress(0)
  setStatus("Idle")
  btnStart.disabled = false
  btnCancel.disabled = true
  btnExport.disabled = true
  btnConvert.disabled = !currentFile
}

// ---- Backend <-> algorithm coupling ----
function populateAlgorithms() {
  const backend = backendSelect.value
  const algorithms = ALGORITHMS[backend] || ALGORITHMS.cpu
  const current = algorithmSelect.value

  algorithmSelect.innerHTML = algorithms.map(
    (alg) => `<option value="${alg}" ${alg === current ? "selected" : ""}>${alg}</option>`
  ).join("")

  if (!algorithms.includes(current)) {
    algorithmSelect.value = algorithms[0]
  }
}

// ---- Quality slider ----
qualityInput.addEventListener("input", (e) => {
  qualityValue.textContent = e.target.value
})

// ---- Convert quality slider ----
convertQuality.addEventListener("input", (e) => {
  convertQualityValue.textContent = e.target.value
})

// ---- File selection (Tauri command -> native file picker) ----
async function selectImage() {
  try {
    const selected = await invoke("select_image")
    if (!selected) return
    const path = typeof selected === "string" ? selected : selected[0]
    await loadImagePath(path)
  } catch (err) {
    console.error("Dialog error:", err)
    setStatus("Open dialog failed: " + (err?.message ?? String(err)), "err")
  }
}

async function loadImagePath(path) {
  try {
    const meta = await invoke("load_image", { path })
    currentFile = {
      path,
      name: path.split(/[/\\]/).pop(),
    }
    fileName.textContent = currentFile.name
    fileName.className = "file-name"

    // Base64 data URL from Rust — no convertFileSrc needed.
    previewImg.src = meta.preview
    previewPlaceholder.classList.add("hidden")
    previewImg.classList.remove("hidden")

    setStatus(
      `Loaded: ${currentFile.name} (${meta.width}×${meta.height}, ${meta.format})`,
      "ok"
    )
    btnExport.disabled = false
    btnConvert.disabled = false
    outputPath = null
  } catch (err) {
    console.error(err)
    setStatus("Load failed: " + (err?.message ?? String(err)), "err")
  }
}

function generateOutputPath(inputPath) {
  const dot = inputPath.lastIndexOf(".")
  const ext = dot === -1 ? "png" : inputPath.slice(dot + 1)
  const base = dot === -1 ? inputPath : inputPath.slice(0, dot)
  return `${base}_upscaled.${ext}`
}

/** Lower-case file extension of `path` (no dot), or "" if none. */
function extOf(path) {
  const name = path.split(/[/\\]/).pop() || ""
  const dot = name.lastIndexOf(".")
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : ""
}

// Save-dialog file types. On Windows, rfd makes the FIRST filter the
// selected "Save as type" and its first extension the default extension, so
// the dialog would otherwise always say PNG and save "x.webp" as "x.webp.png".
const SAVE_FILTERS = [
  ["PNG", "png"],
  ["JPEG", "jpg", "jpeg"],
  ["WebP", "webp"],
  ["GIF", "gif"],
  ["TIFF", "tiff", "tif"],
  ["BMP", "bmp"],
  ["ICO", "ico"],
  ["PNM", "pnm", "pbm", "pgm", "ppm", "pam"],
]

/** SAVE_FILTERS reordered so the filter (and extension) for `ext` comes first. */
function saveFiltersFor(ext) {
  const idx = SAVE_FILTERS.findIndex((f) => f.slice(1).includes(ext))
  if (idx === -1) return SAVE_FILTERS
  const [name, ...exts] = SAVE_FILTERS[idx]
  const preferred = [name, ext, ...exts.filter((e) => e !== ext)]
  return [preferred, ...SAVE_FILTERS.filter((_, i) => i !== idx)]
}

// ---- Wire UI ----
backendSelect.addEventListener("change", populateAlgorithms)
populateAlgorithms()

// Click drop zone to open file picker (Tauri dialog)
dropZone.addEventListener("click", selectImage)

// Drag-over visual feedback (drop itself still opens the dialog, since
// drag-and-drop File objects carry no filesystem path in browsers).
dropZone.addEventListener("dragover", (e) => {
  e.preventDefault()
  dropZone.classList.add("dragover")
  dropZone.classList.remove("drag-error")
})

dropZone.addEventListener("dragleave", () => {
  dropZone.classList.remove("dragover")
})

dropZone.addEventListener("drop", (e) => {
  e.preventDefault()
  dropZone.classList.remove("dragover")
  selectImage()
})

// ---- Start Upscale ----
btnStart.addEventListener("click", async () => {
  if (!currentFile) {
    alert("Please select an image first.")
    return
  }

  const [width, height] = resolutionSelect.value.split("x").map(Number)
  const out = generateOutputPath(currentFile.path)
  outputPath = out

  const payload = {
    job: {
      input_path: currentFile.path,
      output_path: out,
      backend: backendSelect.value,
      algorithm: algorithmSelect.value,
      target_width: width,
      target_height: height,
      jpeg_quality: Number(qualityInput.value) || undefined,
    },
  }

  try {
    // Create a Channel for progress events
    upscaleChannel = new Channel()

    // Listen for progress events from the backend
    upscaleChannel.onmessage = (event) => {
      const data = event.data
      switch (data.event) {
        case "Started":
          currentJobId = data.job_id
          break
        case "Progress":
          setProgress(data.percent)
          setStatus(data.message, "warn")
          break
        case "Completed":
          currentJobId = null
          upscaleChannel = null
          setProgress(100)
          if (data.result.used_fallback && data.result.fallback_reason) {
            setStatus(`Upscale complete (GPU fallback: ${data.result.fallback_reason})`, "ok")
          } else {
            setStatus("Upscale complete.", "ok")
          }
          btnStart.disabled = false
          btnCancel.disabled = true
          btnExport.disabled = false
          break
        case "Cancelled":
          currentJobId = null
          upscaleChannel = null
          setProgress(0)
          setStatus("Cancelled", "warn")
          btnStart.disabled = false
          btnCancel.disabled = true
          btnExport.disabled = true
          break
        case "Failed":
          currentJobId = null
          upscaleChannel = null
          setProgress(0)
          setStatus("Failed: " + data.error, "err")
          btnStart.disabled = false
          btnCancel.disabled = true
          btnExport.disabled = true
          break
      }
    }

    // Start the upscale with the channel
    const result = await invoke("upscale", {
      job: {
        input_path: currentFile.path,
        output_path: generateOutputPath(currentFile.path),
        backend: backendSelect.value,
        algorithm: algorithmSelect.value,
        target_width: width,
        target_height: height,
        jpeg_quality: Number(qualityInput.value) || undefined,
      },
      onEvent: upscaleChannel,
    })

    // The Channel handles all progress, so we don't need to do anything here
    // The onmessage handler will process the Completed event
  } catch (err) {
    console.error("Upscale error:", err)
    setStatus("Upscale failed: " + String(err), "err")
    resetJob()
  }
})

// ---- Cancel ----
btnCancel.addEventListener("click", async () => {
  if (!currentJobId) return
  try {
    await invoke("cancel_job", { id: currentJobId })
  } catch (err) {
    console.error(err)
  }
  // resetJob will be called when the Cancelled event arrives
})

// ---- Export (Save As) ----
btnExport.addEventListener("click", async () => {
  if (!outputPath) {
    alert("No upscaled image to export. Run upscale first.")
    return
  }
  try {
    const defaultName = outputPath.split(/[/\\]/).pop() || "upscaled.png"
    const filters = saveFiltersFor(extOf(defaultName))
    console.log("Calling select_save_path with:", { defaultName, filters })
    const savePath = await invoke("select_save_path", {
      defaultName,
      filters,
    })
    console.log("select_save_path returned:", savePath)
    if (!savePath) {
      setStatus("Save cancelled", "warn")
      return
    }

    const format = extOf(savePath) || "png"
    await invoke("export_image", {
      path: outputPath,
      outputPath: savePath,
      format,
      quality: Number(qualityInput.value),
    })
    setStatus(`Exported to ${savePath}`, "ok")
    alert(`Image exported to ${savePath}`)
  } catch (err) {
    console.error("Export error:", err)
    setStatus("Export failed: " + String(err), "err")
  }
})

// ---- Convert & Save As ----
btnConvert.addEventListener("click", async () => {
  if (!currentFile) {
    alert("Please select an image first.")
    return
  }
  try {
    const defaultName = currentFile.name.replace(/\.[^.]+$/, "") + "." + convertFormat.value
    const filters = saveFiltersFor(extOf(defaultName))
    console.log("Calling select_save_path for convert with:", { defaultName, filters })
    const savePath = await invoke("select_save_path", {
      defaultName,
      filters,
    })
    console.log("select_save_path returned:", savePath)
    if (!savePath) {
      setStatus("Save cancelled", "warn")
      return
    }

    // Use the extension the user actually chose in the dialog so the file
    // content always matches its name; fall back to the dropdown value.
    const format = extOf(savePath) || convertFormat.value
    // Tauri maps top-level Rust snake_case args to camelCase on the JS side.
    await invoke("convert_image", {
      inputPath: currentFile.path,
      outputPath: savePath,
      format,
      quality: Number(convertQuality.value),
    })
    setStatus(`Converted & saved to ${savePath}`, "ok")
    alert(`Image converted & saved to ${savePath}`)
  } catch (err) {
    console.error("Convert error:", err)
    setStatus("Convert failed: " + String(err), "err")
  }
})

// ---- Init ----
resetJob()