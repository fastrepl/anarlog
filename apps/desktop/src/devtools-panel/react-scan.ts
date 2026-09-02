export async function enableReactScan(enabled: boolean) {
  if (!enabled) {
    return;
  }

  try {
    const { scan } = await import("react-scan");
    scan({
      enabled: true,
      showToolbar: true,
      dangerouslyForceRunInProduction: !import.meta.env.DEV,
    });
  } catch (error) {
    console.warn("Failed to start React Scan:", error);
  }
}
