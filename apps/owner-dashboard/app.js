const $ = (id) => document.getElementById(id);

async function api(path, options = {}) {
  const base = $("apiBase").value.replace(/\/$/, "");
  const token = $("token").value.trim();
  const response = await fetch(`${base}${path}`, {
    ...options,
    headers: {
      accept: "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(options.body ? { "content-type": "application/json" } : {}),
      ...(options.headers || {}),
    },
  });
  const text = await response.text();
  let body;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = text;
  }
  if (!response.ok) {
    throw new Error(`${response.status}: ${typeof body === "string" ? body : JSON.stringify(body)}`);
  }
  return body;
}

function show(id, value) {
  $(id).textContent =
    typeof value === "string" ? value : JSON.stringify(value, null, 2);
}

async function refresh() {
  $("status").textContent = "Loading…";
  try {
    const [organization, branches, devices, syncHealth, sales, backups] =
      await Promise.all([
        api("/v1/owner/organization"),
        api("/v1/owner/branches"),
        api("/v1/owner/devices"),
        api("/v1/owner/sync-health"),
        api("/v1/owner/sales/summary"),
        api("/v1/owner/backups"),
      ]);
    show("organization", organization);
    show("branches", branches);
    show("devices", devices);
    show("syncHealth", syncHealth);
    show("sales", sales);
    show("backups", backups);
    $("status").textContent = "Updated";
  } catch (error) {
    $("status").textContent = error.message;
  }
}

async function revokeDevice() {
  const deviceId = $("revokeDeviceId").value.trim();
  if (!deviceId) {
    $("status").textContent = "Enter a device ID to revoke.";
    return;
  }
  $("status").textContent = "Revoking…";
  try {
    const result = await api(`/v1/owner/devices/${encodeURIComponent(deviceId)}/revocations`, {
      method: "POST",
      body: JSON.stringify({ reason: "Owner revoked from dashboard" }),
    });
    show("devices", result);
    $("status").textContent = "Device revocation recorded";
  } catch (error) {
    $("status").textContent = error.message;
  }
}

$("refresh").addEventListener("click", refresh);
$("revoke").addEventListener("click", revokeDevice);
