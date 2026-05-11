export function wireProfileModalOrchestrationImpl(root, model, rerender, t, deps) {
  const {
    openProfileModal
  } = deps;

  root.querySelector("#profile-create")?.addEventListener("click", () => openProfileModal(root, model, rerender, t, null));
}
