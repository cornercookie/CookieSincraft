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
// Batch elements
const btnSelectBatch = $("btnSelectBatch")
const btnStartBatch = $("btnStartBatch")
const batchFileList = $("batchFileList")
const batchProgressFill = $("batchProgressFill")
const batchProgressPct = $("batchProgressPct")
const batchStatus = $("batchStatus")
// Settings elements
const keepAspectRatio = $("keepAspectRatio")
const outputFolder = $("outputFolder")
const btnChooseOutputFolder = $("btnChooseOutputFolder")
const filenameTemplate = $("filenameTemplate")
const recentFilesEl = $("recentFiles")

// Toast container (R4)
const toastContainer = $("toastContainer")

// ---- Algorithm options by backend ----
const ALGORITHMS = {
  cpu: ["Nearest", "Bilinear", "CatmullRom", "Lanczos3"],
  gpu: ["FSR 1.0", "Anime4K", "NIS"],
}

// ---- State ----
let currentFile = null   // { path, name, width, height }
let currentJobId = null
let outputPath = null
let upscaleChannel = null
let settings = {
  backend: "cpu",
  algorithm: "Lanczos3",
  quality: 85,
  output_folder: null,
  keep_aspect_ratio: false,
  filename_template: "{name}_upscaled_{scale}x.{ext}",
  recent_files: [],
}
let batchFiles = []
let batchChannel = null

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

// ---- Settings persistence ----
async function loadSettings() {
  try {
    settings = await invoke("read_settings")
    backendSelect.value = settings.backend || "cpu"
    algorithmSelect.value = settings.algorithm || "Lanczos3"
    qualityInput.value = settings.quality ?? 85
    qualityValue.textContent = qualityInput.value
    outputFolder.value = settings.output_folder || ""
    filenameTemplate.value = settings.filename_template || "{name}_upscaled_{scale}x.{ext}"
    keepAspectRatio.checked = settings.keep_aspect_ratio || false
    populateAlgorithms()
    renderRecentFiles(settings.recent_files || [])
  } catch (err) {
    console.error("Load settings failed:", err)
  }
}

async function saveSettings() {
  try {
    settings = {
      ...settings,
      backend: backendSelect.value,
      algorithm: algorithmSelect.value,
      quality: Number(qualityInput.value),
      output_folder: outputFolder.value || null,
      keep_aspect_ratio: keepAspectRatio.checked,
      filename_template: filenameTemplate.value,
    }
    await invoke("write_settings", settings)
  } catch (err) {
    console.error("Save settings failed:", err)
    setStatus("Save settings failed: " + String(err), "err")
  }
}

async function addRecentFile(path) {
  try {
    settings = await invoke("add_recent_file", { path })
    renderRecentFiles(settings.recent_files || [])
  } catch (err) {
    console.error("Add recent file failed:", err)
  }
}

function renderRecentFiles(files) {
  recentFilesEl.innerHTML = ""
  if (!files || files.length === 0) {
    recentFilesEl.textContent = "(none)"
    recentFilesEl.className = "recent-files empty"
    return
  }
  recentFilesEl.className = "recent-files"
  const list = document.createElement("div")
  list.className = "recent-list"
  for (const path of files) {
    const item = document.createElement("div")
    item.className = "recent-item"
    item.title = path
    item.textContent = path.split(/[/\\]/).pop()
    item.style.cursor = "pointer"
    item.onclick = async () => {
      try {
        await loadImagePath(path)
      } catch (err) {
        showToast("Failed to load: " + String(err), "err")
      }
    }
    list.appendChild(item)
  }
  recentFilesEl.appendChild(list)
}

// Resolve output path from settings: output_folder + filename_template.
// Sanitizes the template to block path traversal.
function resolveOutputPath(inputPath) {
  const outFolder = settings.output_folder
  const template = settings.filename_template || "{name}_upscaled_{scale}x.{ext}"

  const inputName = inputPath.split(/[/\\]/).pop()
  const dot = inputName.lastIndexOf(".")
  const name = dot === -1 ? inputName : inputName.slice(0, dot)
  const ext = dot === -1 ? "png" : inputName.slice(dot + 1)

  const [tw, th] = resolutionSelect.value.split("x").map(Number)
  const scale = keepAspectRatio.checked ? Math.min(tw, th) : tw

  let filename = template
    .replace(/\{name\}/g, name)
    .replace(/\{scale\}/g, scale)
    .replace(/\{ext\}/g, ext)

  // Sanitize: block path traversal characters (Security X3)
  filename = filename.replace(/[\\/:*?"<>|]/g, "_")
  if (filename === ".." || filename.includes("..")) filename = "output." + ext

  if (outFolder) {
    return outFolder.endsWith("\\") || outFolder.endsWith("/")
      ? outFolder + filename
      : outFolder + "/" + filename
  }
  const lastSep = Math.max(inputPath.lastIndexOf("/"), inputPath.lastIndexOf("\\"))
  const dir = lastSep === -1 ? "" : inputPath.slice(0, lastSep + 1)
  return dir + filename
}

// ---- Batch upscale ----
function renderBatchFileList() {
  batchFileList.innerHTML = ""
  if (!batchFiles || batchFiles.length === 0) {
    batchFileList.textContent = "(none)"
    batchFileList.className = "batch-file-list empty"
    btnStartBatch.disabled = true
    return
  }
  batchFileList.className = "batch-file-list"
  const list = document.createElement("div")
  list.className = "batch-list"
  for (const path of batchFiles) {
    const item = document.createElement("div")
    item.className = "batch-item"
    item.title = path
    item.textContent = path.split(/[/\\]/).pop()
    list.appendChild(item)
  }
  batchFileList.appendChild(list)
  btnStartBatch.disabled = false
}

async function startBatchUpscale() {
  if (batchFiles.length === 0) return
  const [width, height] = resolutionSelect.value.split("x").map(Number)

  const items = batchFiles.map(inputPath => ({
    input_path: inputPath,
    output_path: null, // backend resolves via output_folder + filename_template
  }))

  try {
    batchChannel = new Channel()
    batchChannel.onmessage = (event) => {
      const data = event.data
      switch (data.event) {
        case "Started":
          batchStatus.textContent = `Batch started: ${data.total} items`
          break
        case "Progress": {
          const pct = Math.round((data.done / data.total) * 100)
          batchProgressFill.style.width = pct + "%"
          batchProgressPct.textContent = pct + "%"
          batchStatus.textContent = `Processing: ${data.current.split(/[/\\]/).pop()}`
          break
        }
        case "ItemCompleted":
          batchStatus.textContent = `Completed: ${data.current.split(/[/\\]/).pop()}`
          break
        case "ItemFailed":
          batchStatus.textContent = `Failed: ${data.current.split(/[/\\]/).pop()}`
          break
        case "Completed":
          batchChannel = null
          batchProgressFill.style.width = "100%"
          batchProgressPct.textContent = "100%"
          batchStatus.textContent = `Batch done: ${data.successes} succeeded, ${data.failures} failed`
          btnStartBatch.disabled = false
          btnSelectBatch.disabled = false
          break
        case "Cancelled":
          batchChannel = null
          batchStatus.textContent = "Batch cancelled"
          btnStartBatch.disabled = false
          btnSelectBatch.disabled = false
          break
        case "Failed":
          batchChannel = null
          batchStatus.textContent = "Batch failed: " + data.error
          btnStartBatch.disabled = false
          btnSelectBatch.disabled = false
          break
      }
    }

    btnStartBatch.disabled = true
    btnSelectBatch.disabled = true

    await invoke("batch_upscale", {
      items,
      backend: backendSelect.value,
      algorithm: algorithmSelect.value,
      keep_aspect_ratio: keepAspectRatio.checked,
      target_width: width,
      target_height: height,
      quality: Number(qualityInput.value) || undefined,
    }, batchChannel)
  } catch (err) {
    console.error("Batch upscale error:", err)
    batchStatus.textContent = "Batch failed: " + String(err)
    btnStartBatch.disabled = false
    btnSelectBatch.disabled = false
    batchChannel = null
  }
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

// ---- Toast notifications (R4) ----
function showToast(message, kind = "info", duration = 4000) {
  const toast = document.createElement("div")
  toast.className = `toast toast-${kind}`
  toast.textContent = message  // XSS-safe: textContent not innerHTML
  toastContainer.appendChild(toast)
  setTimeout(() => toast.remove(), duration)
}

// ---- Overwrite confirmation (R2) ----
async function confirmOverwrite(outputPath) {
  const exists = await invoke("check_path_exists", { path: outputPath })
  if (!exists) return { action: "overwrite", finalPath: outputPath }

  return new Promise((resolve) => {
    const overlay = document.createElement("div")
    overlay.className = "modal-overlay"
    const modal = document.createElement("div")
    modal.className = "modal"

    // XSS-safe: use textContent for user-controlled strings
    const fileName = outputPath.split(/[/\\]/).pop()
    const p1 = document.createElement("p")
    p1.textContent = `檔案已存在：${fileName}`
    const p2 = document.createElement("p")
    p2.textContent = "要如何處理？"
    modal.appendChild(p1)
    modal.appendChild(p2)

    const actions = document.createElement("div")
    actions.className = "modal-actions"

    const btnOverwrite = document.createElement("button")
    btnOverwrite.id = "ovrOverwrite"
    btnOverwrite.className = "btn btn-primary"
    btnOverwrite.textContent = "覆寫"
    btnOverwrite.onclick = () => {
      overlay.remove()
      resolve({ action: "overwrite", finalPath: outputPath })
    }

    const btnRename = document.createElement("button")
    btnRename.id = "ovrRename"
    btnRename.className = "btn btn-secondary"
    btnRename.textContent = "自動改名"
    btnRename.onclick = async () => {
      try {
        const newPath = await generateUniquePath(outputPath)
        overlay.remove()
        resolve({ action: "rename", finalPath: newPath })
      } catch (e) {
        showToast("無法產生新檔名", "err")
      }
    }

    const btnCancel = document.createElement("button")
    btnCancel.id = "ovrCancel"
    btnCancel.className = "btn btn-outline"
    btnCancel.textContent = "取消"
    btnCancel.onclick = () => {
      overlay.remove()
      resolve({ action: "cancel" })
    }

    actions.appendChild(btnOverwrite)
    actions.appendChild(btnRename)
    actions.appendChild(btnCancel)
    modal.appendChild(actions)
    overlay.appendChild(modal)
    document.body.appendChild(overlay)
  })
}

// Generate unique path with _upscaled_N suffix (max 100 attempts)
// Respects backend rate limit of 20 requests/minute (3 second delay between requests)
async function generateUniquePath(basePath) {
  const dot = basePath.lastIndexOf(".")
  const ext = dot === -1 ? "" : basePath.slice(dot)
  const base = dot === -1 ? basePath : basePath.slice(0, dot)
  for (let n = 1; n <= 100; n++) {
    const candidate = `${base}_upscaled_${n}${ext}`
    const exists = await invoke("check_path_exists", { path: candidate })
    if (!exists) return candidate
    // Delay to respect 20/min rate limit (3 seconds between requests)
    await new Promise(r => setTimeout(r, 3000))
  }
  throw new Error("Unable to generate unique path after 100 attempts")
}

// Load image from base64 (drag-and-drop, R1)
async function loadImageFromBase64(file) {
  // Frontend size check: 50 MB limit (Q7)
  const MAX_FILE_SIZE = 50 * 1024 * 1024
  if (file.size > MAX_FILE_SIZE) {
    showToast(`檔案過大：${(file.size / 1024 / 1024).toFixed(1)} MB > 50 MB 限制`, "err")
    return
  }

  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = async (e) => {
      try {
        const base64 = e.target.result.split(",")[1] // strip data URL prefix
        const result = await invoke("load_image_from_base64", {
          base64,
          suggestedName: file.name,
        })
        currentFile = {
          path: result.temp_path,
          name: file.name,
          width: result.image_meta.width,
          height: result.image_meta.height,
        }
        fileName.textContent = currentFile.name
        fileName.className = "file-name"
        previewImg.src = result.image_meta.preview
        previewPlaceholder.classList.add("hidden")
        previewImg.classList.remove("hidden")
        setStatus(
          `Loaded: ${currentFile.name} (${result.image_meta.width}×${result.image_meta.height}, ${result.image_meta.format})`,
          "ok"
        )
        btnExport.disabled = false
        btnConvert.disabled = false
        outputPath = null
        await addRecentFile(result.temp_path)
        resolve()
      } catch (err) {
        console.error(err)
        setStatus("Load failed: " + (err?.message ?? String(err)), "err")
        reject(err)
      }
    }
    reader.onerror = () => reject(new Error("File read failed"))
    reader.readAsDataURL(file)
  })
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
      width: meta.width,
      height: meta.height,
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
    await addRecentFile(path)
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

// File input change handler (alternative to drag-drop, R1)
const fileInput = $("fileInput")
fileInput.addEventListener("change", (e) => {
  const file = e.target.files?.[0]
  if (file && file.type.startsWith("image/")) {
    loadImageFromBase64(file)
  }
  // Reset value so same file can be selected again
  fileInput.value = ""
})

// Drag-over visual feedback
dropZone.addEventListener("dragover", (e) => {
  e.preventDefault()
  dropZone.classList.add("dragover")
  dropZone.classList.remove("drag-error")
})

dropZone.addEventListener("dragleave", () => {
  dropZone.classList.remove("dragover")
})

// Drop handler: read file as base64 and load via load_image_from_base64 (R1)
dropZone.addEventListener("drop", async (e) => {
  e.preventDefault()
  dropZone.classList.remove("dragover")
  dropZone.classList.remove("drag-error")

  const file = e.dataTransfer.files?.[0]
  if (!file || !file.type.startsWith("image/")) {
    dropZone.classList.add("drag-error")
    showToast("請放置圖片檔案", "err")
    return
  }
  await loadImageFromBase64(file)
})

// ---- Choose output folder (native folder picker) ----
btnChooseOutputFolder.addEventListener("click", async () => {
  try {
    const folder = await invoke("select_output_folder")
    if (!folder) return
    outputFolder.value = folder
    await saveSettings()
    setStatus(`Output folder: ${folder}`, "ok")
  } catch (err) {
    console.error("Choose output folder failed:", err)
    setStatus("Choose output folder failed: " + String(err), "err")
  }
})

// ---- Settings change handlers ----
backendSelect.addEventListener("change", () => {
  populateAlgorithms()
  saveSettings()
})
algorithmSelect.addEventListener("change", saveSettings)
qualityInput.addEventListener("change", saveSettings)
outputFolder.addEventListener("change", saveSettings)
filenameTemplate.addEventListener("change", saveSettings)
keepAspectRatio.addEventListener("change", saveSettings)

// ---- Batch events ----
btnSelectBatch.addEventListener("click", async () => {
  try {
    batchFiles = await invoke("select_multiple_images")
    renderBatchFileList()
  } catch (err) {
    console.error("Select multiple images failed:", err)
    setStatus("Select multiple images failed: " + String(err), "err")
  }
})

btnStartBatch.addEventListener("click", startBatchUpscale)

// ---- Start Upscale ----
btnStart.addEventListener("click", async () => {
  if (!currentFile) {
    alert("請先選擇圖片")
    return
  }

  const [width, height] = resolutionSelect.value.split("x").map(Number)

  let targetWidth = width
  let targetHeight = height

  // Keep aspect ratio: frontend pre-calculates by short side (Design: 前端預計算，後端僅標記)
  if (keepAspectRatio.checked && currentFile.width && currentFile.height) {
    const targetShort = Math.min(width, height)
    const ratio = currentFile.width / currentFile.height
    if (currentFile.width >= currentFile.height) {
      targetHeight = targetShort
      targetWidth = Math.round(targetShort * ratio)
    } else {
      targetWidth = targetShort
      targetHeight = Math.round(targetShort / ratio)
    }
    // Clamp to 4096×4096 limit (validate_dimensions)
    if (targetWidth > 4096) targetWidth = 4096
    if (targetHeight > 4096) targetHeight = 4096
  }

  const out = resolveOutputPath(currentFile.path)

  // Overwrite confirmation (R2)
  const overwriteResult = await confirmOverwrite(out)
  if (overwriteResult.action === "cancel") {
    setStatus("Cancelled", "warn")
    return
  }
  outputPath = overwriteResult.finalPath

  const payload = {
    job: {
      input_path: currentFile.path,
      output_path: outputPath,
      backend: backendSelect.value,
      algorithm: algorithmSelect.value,
      target_width: targetWidth,
      target_height: targetHeight,
      jpeg_quality: Number(qualityInput.value) || undefined,
      keep_aspect_ratio: keepAspectRatio.checked,
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
          // Update preview with upscaled result (R3)
          if (data.result.preview) {
            previewImg.src = data.result.preview
          }
          // Toast notification on fallback (R4)
          if (data.result.used_fallback && data.result.fallback_reason) {
            setStatus(`Upscale complete (GPU fallback: ${data.result.fallback_reason})`, "ok")
            showToast(`已改用 CPU 放大：${data.result.fallback_reason}`, "warn")
          } else {
            setStatus("Upscale complete.", "ok")
          }
          btnStart.disabled = false
          btnCancel.disabled = true
          btnExport.disabled = false
          saveSettings()
          addRecentFile(data.result.output_path)
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
        output_path: outputPath,
        backend: backendSelect.value,
        algorithm: algorithmSelect.value,
        target_width: targetWidth,
        target_height: targetHeight,
        jpeg_quality: Number(qualityInput.value) || undefined,
        keep_aspect_ratio: keepAspectRatio.checked,
      },
      onEvent: upscaleChannel,
    })

    // The Channel handles all progress, so we don't need to do anything here
    // The onmessage handler will process the Completed event
  } catch (err) {
    console.error("Upscale error:", err)
    setStatus("Upscale failed: " + String(err), "err")
    upscaleChannel = null  // Clear channel on error
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
    const savePath = await invoke("select_save_path", {
      defaultName,
      filters,
    })
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
    saveSettings()
    addRecentFile(savePath)
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
    const savePath = await invoke("select_save_path", {
      defaultName,
      filters,
    })
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
loadSettings()