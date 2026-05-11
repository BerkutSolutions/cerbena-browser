import { askConfirmModal, askInputModal } from "../../core/modal.js";
import { exportProfile } from "./api.js";

export async function askInputPrompt(t, title, label, defaultValue = "") {
  return askInputModal(t, {
    title,
    label,
    defaultValue
  });
}

export async function askConfirmPrompt(t, title, description, descriptionHtml = "") {
  return askConfirmModal(t, {
    title,
    description,
    descriptionHtml
  });
}

export async function exportProfileArchiveAction(model, t, profileId, setNotice) {
  const passphrase = await askInputPrompt(t, t("profile.export.title"), t("profile.export.passphrase"));
  if (!passphrase) return false;
  const data = await exportProfile(profileId, passphrase);
  if (data.ok) {
    await navigator.clipboard?.writeText(data.data.archive_json);
    setNotice("success", t("profile.export.copied"));
  } else {
    setNotice("error", String(data.data.error));
  }
  return true;
}
