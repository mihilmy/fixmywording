const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const apiKeyInput = document.getElementById("api-key");
const modelSelect = document.getElementById("model");
const systemPromptInput = document.getElementById("system-prompt");
const resetPromptBtn = document.getElementById("reset-prompt");

// Window dragging
document.getElementById("titlebar").addEventListener("mousedown", (e) => {
  if (e.buttons === 1) getCurrentWindow().startDragging();
});

// Auto-save with debounce
let saveTimeout;
function autoSave() {
  clearTimeout(saveTimeout);
  saveTimeout = setTimeout(async () => {
    try {
      await invoke("save_config", {
        config: {
          api_key: apiKeyInput.value.trim(),
          model: modelSelect.value,
          system_prompt: systemPromptInput.value.trim(),
        },
      });
      showToast("Saved", "success");
    } catch (err) {
      showToast("Failed to save: " + err, "error");
    }
  }, 500);
}

apiKeyInput.addEventListener("input", autoSave);
modelSelect.addEventListener("change", autoSave);
systemPromptInput.addEventListener("input", autoSave);

// Load settings
async function loadSettings() {
  try {
    const config = await invoke("get_config");
    apiKeyInput.value = config.api_key || "";
    modelSelect.value = config.model || "claude-sonnet-4-5-20250929";
    systemPromptInput.value = config.system_prompt || "";
  } catch (e) {
    console.error("Failed to load settings:", e);
  }
}

// Toast
function showToast(message, type = "success") {
  const toast = document.getElementById("toast");
  const msg = document.getElementById("toast-message");
  const iconSuccess = document.getElementById("toast-icon-success");
  const iconError = document.getElementById("toast-icon-error");

  msg.textContent = message;
  iconSuccess.classList.toggle("hidden", type !== "success");
  iconError.classList.toggle("hidden", type !== "error");

  toast.classList.remove("hidden");
  toast.firstElementChild.className = toast.firstElementChild.className.replace("toast-exit", "");
  toast.firstElementChild.classList.add("toast-enter");

  setTimeout(() => {
    toast.firstElementChild.classList.remove("toast-enter");
    toast.firstElementChild.classList.add("toast-exit");
    setTimeout(() => toast.classList.add("hidden"), 150);
  }, 2000);
}

resetPromptBtn.addEventListener("click", async () => {
  try {
    const defaults = await invoke("get_default_config");
    console.log(defaults);
    systemPromptInput.value = defaults.system_prompt;
    autoSave();
  } catch (err) {
    showToast("Reset failed: " + err, "error");
  }
});

loadSettings();
