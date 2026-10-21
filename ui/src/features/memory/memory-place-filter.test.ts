import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";

import { sampleNotes } from "$lib/ipc/mock/memory-notes-fixtures";
import { sampleScripts } from "$lib/ipc/mock/memory-scripts-fixtures";
import { nav } from "$lib/stores/nav.svelte";
import { viewState } from "$lib/stores/view-state.svelte";

import {
  calls,
  fakeMemory,
  filterChoices,
  noteRow,
  pickFilter,
  registerMemoryHooks,
  rowButton,
  showMemory,
  showNotes,
} from "./memory-test-support";
import NotesView from "./NotesView.svelte";

registerMemoryHooks();

const inPlace = <T extends { place: "team" | "local" }>(
  items: T[],
  place: T["place"],
) => items.filter((item) => item.place === place).length;

/** What a request to the server carried under one name; a filter not asked for is not there. */
function sent(asked: unknown, name: string): unknown {
  return (asked as Record<string, unknown>)[name];
}

describe("the Kept in filter of the notes", () => {
  const [local, team] = [
    inPlace(sampleNotes(), "local"),
    inPlace(sampleNotes(), "team"),
  ];

  it("lists both places with their counts, right after the Scope filter", async () => {
    fakeMemory();
    await showNotes();

    expect(await filterChoices("Kept in: All", "Kept in")).toEqual([
      `All ${local + team}`,
      `This computer ${local}`,
      `Remote ${team}`,
    ]);

    const scope = screen.getByRole("button", { name: "Scope: All" });
    const place = screen.getByRole("button", { name: "Kept in: All" });
    expect(
      scope.compareDocumentPosition(place) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("is there when everything is kept in one place too, as the column is", async () => {
    const notes = sampleNotes().map((note) => ({ ...note, place: "local" as const }));
    fakeMemory({ kind: "available" }, undefined, undefined, notes);
    await showNotes();

    expect(await filterChoices("Kept in: All", "Kept in")).toEqual([
      `All ${notes.length}`,
      `This computer ${notes.length}`,
      "Remote 0",
    ]);
  });

  it("is not there on a Harness Memory that cannot filter by place", async () => {
    fakeMemory({ kind: "available" }, undefined, undefined, undefined, false);
    await showNotes();

    await screen.findByRole("button", { name: "Kind: All" });
    expect(screen.queryByRole("button", { name: /^Kept in: / })).toBeNull();
  });

  it("asks for the remote notes alone from the first page, and clears back to both", async () => {
    const invoke = fakeMemory();
    viewState.notesPage = 1;
    await showNotes();
    const lastAsk = () => calls(invoke, "memory_notes_list").at(-1);

    expect(sent(lastAsk(), "place")).toBeUndefined();

    await pickFilter("Kept in: All", /^Remote/);

    await waitFor(() =>
      expect(lastAsk()).toMatchObject({ place: "team", offset: 0 }),
    );
    await waitFor(() =>
      expect(screen.getByText(`${team} notes`)).toBeInTheDocument(),
    );
    expect(viewState.notesPlace).toBe("team");
    expect(
      noteRow(sampleNotes().find((note) => note.place === "local")!.id),
    ).toBeNull();

    await fireEvent.click(
      screen.getByRole("button", { name: "Clear the Kept in filter" }),
    );

    await waitFor(() => expect(sent(lastAsk(), "place")).toBeUndefined());
    await waitFor(() =>
      expect(screen.getByText(`${local + team} notes`)).toBeInTheDocument(),
    );
    expect(viewState.notesPlace).toBeNull();
  });

  it("says nothing matches a place that holds none, and goes back to All when cleared", async () => {
    fakeMemory(
      { kind: "available" },
      undefined,
      undefined,
      sampleNotes().map((note) => ({ ...note, place: "local" as const })),
    );
    viewState.notesPlace = "team";
    nav.go("notes");
    render(NotesView);

    expect(
      await screen.findByText("No notes match these filters."),
    ).toBeInTheDocument();

    await fireEvent.click(
      screen.getByRole("button", { name: "Clear the Kept in filter" }),
    );

    expect(
      await screen.findByRole("grid", { name: "Notes" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kept in: All" })).toBeInTheDocument();
  });

  it("does not count a place chosen before an older Harness Memory as a filter", async () => {
    fakeMemory({ kind: "available" }, undefined, undefined, undefined, false);
    viewState.notesPlace = "team";
    await showNotes();

    expect(screen.queryByText("No notes match these filters.")).toBeNull();
    expect(screen.queryByRole("button", { name: /^Kept in: / })).toBeNull();
  });
});

describe("the Kept in filter of the scripts", () => {
  const [local, team] = [
    inPlace(sampleScripts(), "local"),
    inPlace(sampleScripts(), "team"),
  ];

  it("lists both places with their counts, right after the Scope filter", async () => {
    fakeMemory();
    await showMemory();

    expect(await filterChoices("Kept in: All", "Kept in")).toEqual([
      `All ${local + team}`,
      `This computer ${local}`,
      `Remote ${team}`,
    ]);

    const scope = screen.getByRole("button", { name: "Scope: All" });
    const place = screen.getByRole("button", { name: "Kept in: All" });
    expect(
      scope.compareDocumentPosition(place) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("is there when everything is kept in one place too, as the column is", async () => {
    const scripts = sampleScripts().map((script) => ({ ...script, place: "team" as const }));
    fakeMemory({ kind: "available" }, scripts);
    await showMemory();

    expect(await filterChoices("Kept in: All", "Kept in")).toEqual([
      `All ${scripts.length}`,
      "This computer 0",
      `Remote ${scripts.length}`,
    ]);
  });

  it("is not there on a Harness Memory that cannot filter by place", async () => {
    fakeMemory({ kind: "available" }, undefined, undefined, undefined, false);
    await showMemory();

    await screen.findByRole("button", { name: "Show: All" });
    expect(screen.queryByRole("button", { name: /^Kept in: / })).toBeNull();
  });

  it("asks for the scripts of this computer alone from the first page, and clears back to both", async () => {
    const invoke = fakeMemory();
    viewState.scriptsPage = 1;
    await showMemory();
    const lastAsk = () => calls(invoke, "memory_scripts_list").at(-1);
    const [onThisComputer, remote] = [
      sampleScripts().find((script) => script.place === "local")!,
      sampleScripts().find((script) => script.place === "team")!,
    ];

    await pickFilter("Kept in: All", /^This computer/);

    await waitFor(() =>
      expect(lastAsk()).toMatchObject({ place: "local", offset: 0 }),
    );
    await waitFor(() => expect(rowButton(remote.name)).toBeNull());
    expect(rowButton(onThisComputer.name)).not.toBeNull();
    expect(viewState.scriptsPlace).toBe("local");

    await fireEvent.click(
      screen.getByRole("button", { name: "Clear the Kept in filter" }),
    );

    await waitFor(() => expect(rowButton(remote.name)).not.toBeNull());
    expect(sent(lastAsk(), "place")).toBeUndefined();
    expect(viewState.scriptsPlace).toBeNull();
  });

  it("counts the place under the other filters", async () => {
    fakeMemory();
    await showMemory();

    const trusted = sampleScripts().filter((script) => script.trusted);
    await pickFilter("Status: All", /^Trusted/);
    await waitFor(() =>
      expect(screen.getByText(`${trusted.length} scripts`)).toBeInTheDocument(),
    );

    expect(await filterChoices("Kept in: All", "Kept in")).toEqual([
      `All ${trusted.length}`,
      `This computer ${inPlace(trusted, "local")}`,
      `Remote ${inPlace(trusted, "team")}`,
    ]);

    // The scripts that need review are all kept remotely: the menu stays, with none on this computer.
    await pickFilter("Status: Trusted", /^Needs review/);

    await waitFor(() =>
      expect(screen.getByText("2 scripts")).toBeInTheDocument(),
    );
    expect(await filterChoices("Kept in: All", "Kept in")).toEqual([
      "All 2",
      "This computer 0",
      "Remote 2",
    ]);
  });
});
