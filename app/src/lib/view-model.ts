import { commands, events, type ViewModel } from "$lib/bindings";

/**
 * Calls `show` with the view model as it is now, then with each one the core pushes. Returns a
 * function that stops listening.
 */
export function followViewModel(show: (view: ViewModel) => void): () => void {
  let pushed = false;
  // Listen before asking, so no push between the two is missed.
  const unlisten = events.viewModelChanged.listen((event) => {
    pushed = true;
    show(event.payload);
  });
  commands
    .viewModel()
    .then((view) => {
      if (!pushed) show(view);
    })
    .catch((e: unknown) => {
      console.error("Could not load the view model", e);
    });
  return () => {
    unlisten.then((stop) => stop());
  };
}
