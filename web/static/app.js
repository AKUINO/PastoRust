const simulationControls = document.querySelector("#simulation-controls");
const gpio = document.querySelector("#gpio15");
const gpioValue = document.querySelector("#gpio-value");
const adc = document.querySelector("#adc26");
const adcValue = document.querySelector("#adc-value");
const status = document.querySelector("#status");
let currentMode = null;
let stateRequestInFlight = false;

function showState(state) {
    currentMode = state.mode;
    gpioValue.textContent = state.gpio15 ? "Haut" : "Bas";
    adcValue.textContent = state.adc26;

    if (state.mode === "simulation") {
        simulationControls.hidden = false;
        gpio.checked = state.gpio15;
        adc.value = state.adc26;
    } else {
        simulationControls.hidden = true;
    }
}

async function updateState() {
    status.textContent = "Enregistrement des valeurs simulées…";
    try {
        const response = await fetch("/api/state", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                gpio15: gpio.checked,
                adc26: Number(adc.value),
            }),
        });
        if (!response.ok) {
            throw new Error(`Réponse HTTP ${response.status}`);
        }
        showState(await response.json());
        status.textContent = "Valeurs simulées enregistrées.";
    } catch (error) {
        status.textContent = `Erreur de communication : ${error.message}`;
    }
}

async function loadState() {
    if (stateRequestInFlight) {
        return;
    }

    stateRequestInFlight = true;
    try {
        const response = await fetch("/api/state", {
            signal: AbortSignal.timeout(5000),
        });
        if (!response.ok) {
            throw new Error(`Réponse HTTP ${response.status}`);
        }
        showState(await response.json());
        status.textContent = "Application connectée.";
    } catch (error) {
        status.textContent = `Erreur de communication : ${error.message}`;
    } finally {
        stateRequestInFlight = false;
    }
}

gpio.addEventListener("change", updateState);
adc.addEventListener("input", () => {
    adcValue.textContent = adc.value;
});
adc.addEventListener("change", updateState);

loadState();
setInterval(() => {
    if (currentMode !== "simulation") {
        loadState();
    }
}, 1000);
