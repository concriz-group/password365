import { act, render, screen, renderHook, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useVaultData } from "../hooks/useVaultData";
import { fakeClient } from "../test/fakeClient";
import { categories, personal, summary, work } from "../test/fixtures";
import { ALL } from "../lib/scope";
import { ItemList } from "./ItemList";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("lists vaults with counts and selects them", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<Sidebar vaults={[personal, work]} selected={{ type: "vault", id: "v1" }} onSelect={onSelect} onCreate={vi.fn()} />);
    expect(screen.getByRole("button", { name: /All items/ })).toHaveTextContent("1");
    expect(screen.getByRole("button", { name: /Personal/ })).toHaveClass("active");
    await user.click(screen.getByRole("button", { name: /Work/ }));
    expect(onSelect).toHaveBeenCalledWith({ type: "vault", id: "v2" });
    await user.click(screen.getByRole("button", { name: /All items/ }));
    expect(onSelect).toHaveBeenCalledWith(ALL);
  });

  it("creates a vault", async () => {
    const user = userEvent.setup();
    const onCreate = vi.fn().mockResolvedValue(undefined);
    render(<Sidebar vaults={[]} selected={ALL} onSelect={vi.fn()} onCreate={onCreate} />);
    await user.click(screen.getByRole("button", { name: "+ New vault" }));
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
    await user.type(screen.getByLabelText("Vault name"), "Family");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(onCreate).toHaveBeenCalledWith("Family");
    expect(await screen.findByRole("button", { name: "+ New vault" })).toBeInTheDocument();
  });

  it("opens Recently Deleted and highlights it instead of the vault", async () => {
    const user = userEvent.setup();
    const onShowDeleted = vi.fn();
    render(
      <Sidebar
        vaults={[personal]}
        selected={{ type: "vault", id: "v1" }}
        onSelect={vi.fn()}
        onCreate={vi.fn()}
        showingDeleted
        onShowDeleted={onShowDeleted}
      />,
    );
    expect(screen.getByRole("button", { name: /Personal/ })).not.toHaveClass("active");
    expect(screen.getByRole("button", { name: "Recently Deleted" })).toHaveClass("active");
    await user.click(screen.getByRole("button", { name: "Recently Deleted" }));
    expect(onShowDeleted).toHaveBeenCalled();
  });

  it("shows Favorites and tags and selects them", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <Sidebar
        vaults={[personal]}
        tags={[{ name: "dev", count: 2 }]}
        selected={{ type: "tag", tag: "dev" }}
        onSelect={onSelect}
        onCreate={vi.fn()}
      />,
    );
    expect(screen.getByRole("heading", { name: "Tags" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /dev/ })).toHaveTextContent("2");
    expect(screen.getByRole("button", { name: /dev/ })).toHaveClass("active");
    await user.click(screen.getByRole("button", { name: "Favorites" }));
    expect(onSelect).toHaveBeenCalledWith({ type: "favorites" });
    await user.click(screen.getByRole("button", { name: /dev/ }));
    expect(onSelect).toHaveBeenCalledWith({ type: "tag", tag: "dev" });
    await user.click(screen.getByRole("button", { name: "Archive" }));
    expect(onSelect).toHaveBeenCalledWith({ type: "archive" });
  });

  it("opens Watchtower and highlights it instead of the vault", async () => {
    const user = userEvent.setup();
    const onShowWatchtower = vi.fn();
    render(
      <Sidebar
        vaults={[personal]}
        selected={{ type: "vault", id: "v1" }}
        onSelect={vi.fn()}
        onCreate={vi.fn()}
        showingWatchtower
        onShowWatchtower={onShowWatchtower}
      />,
    );
    expect(screen.getByRole("button", { name: /Personal/ })).not.toHaveClass("active");
    expect(screen.getByRole("button", { name: "Watchtower" })).toHaveClass("active");
    await user.click(screen.getByRole("button", { name: "Watchtower" }));
    expect(onShowWatchtower).toHaveBeenCalled();
  });

  it("hides the Tags heading when no item has a tag", () => {
    render(<Sidebar vaults={[]} selected={ALL} onSelect={vi.fn()} onCreate={vi.fn()} />);
    expect(screen.queryByRole("heading", { name: "Tags" })).not.toBeInTheDocument();
  });

  it("can cancel vault creation", async () => {
    const user = userEvent.setup();
    render(<Sidebar vaults={[]} selected={ALL} onSelect={vi.fn()} onCreate={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "+ New vault" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByLabelText("Vault name")).not.toBeInTheDocument();
  });
});

describe("ItemList", () => {
  it("filters by category and changes the sort order", async () => {
    const user = userEvent.setup();
    const onCategory = vi.fn();
    const onSort = vi.fn();
    render(
      <ItemList
        items={[summary]}
        selectedId={null}
        onSelect={vi.fn()}
        query=""
        onQuery={vi.fn()}
        categories={categories.filter((c) => c.kind === "login" || c.kind === "note")}
        category={null}
        onCategory={onCategory}
        sort="title"
        onSort={onSort}
      />,
    );
    const category = screen.getByLabelText("Category");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual([
      "All Categories",
      "Logins",
      "Secure Notes",
      "Title",
      "Date Modified",
    ]);
    await user.selectOptions(category, "note");
    expect(onCategory).toHaveBeenLastCalledWith("note");
    await user.selectOptions(category, "");
    expect(onCategory).toHaveBeenLastCalledWith(null);
    await user.selectOptions(screen.getByLabelText("Sort by"), "updated");
    expect(onSort).toHaveBeenCalledWith("updated");
  });

  it("keeps the chosen category in the menu when no item of it is left", () => {
    render(
      <ItemList
        items={[]}
        selectedId={null}
        onSelect={vi.fn()}
        query=""
        onQuery={vi.fn()}
        categories={categories.slice(0, 1)}
        category="note"
        onCategory={vi.fn()}
      />,
    );
    expect(screen.getByLabelText("Category")).toHaveValue("note");
    expect(screen.queryByLabelText("Sort by")).not.toBeInTheDocument();
  });

  it("marks items to review", () => {
    render(
      <ItemList items={[{ ...summary, needs_review: true }]} selectedId={null} onSelect={vi.fn()} query="" onQuery={vi.fn()} />,
    );
    expect(screen.getByText("Review")).toBeInTheDocument();
  });

  it("shows items and reports selection and search", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    const onQuery = vi.fn();
    const noUser = { ...summary, id: "i2", title: "Wifi", username: null, urls: [] };
    render(<ItemList items={[summary, noUser]} selectedId="i1" onSelect={onSelect} query="" onQuery={onQuery} />);
    expect(screen.getByRole("button", { name: /GitHub/ })).toHaveClass("active");
    expect(screen.getByRole("button", { name: /Wifi/ })).toHaveTextContent("login");
    await user.click(screen.getByRole("button", { name: /Wifi/ }));
    expect(onSelect).toHaveBeenCalledWith("i2");
    await user.type(screen.getByLabelText("Search"), "g");
    expect(onQuery).toHaveBeenCalledWith("g");
  });

  it("shows empty states", () => {
    const { rerender } = render(
      <ItemList items={[]} selectedId={null} onSelect={vi.fn()} query="" onQuery={vi.fn()} />,
    );
    expect(screen.getByText("No items yet.")).toBeInTheDocument();
    rerender(<ItemList items={[]} selectedId={null} onSelect={vi.fn()} query="x" onQuery={vi.fn()} />);
    expect(screen.getByText("No matching items.")).toBeInTheDocument();
  });
});

describe("useVaultData", () => {
  it("loads and reloads with filters", async () => {
    const client = fakeClient({
      vaults: vi.fn().mockResolvedValue([personal]),
      items: vi.fn().mockResolvedValue([summary]),
    });
    const { result } = renderHook(() => useVaultData(client));
    await waitFor(() => expect(result.current.items).toEqual([summary]));
    expect(result.current.vaults).toEqual([personal]);

    result.current.setVaultId("v1");
    await waitFor(() => expect(client.items).toHaveBeenLastCalledWith("", "v1"));
    result.current.setQuery("git");
    await waitFor(() => expect(client.items).toHaveBeenLastCalledWith("git", "v1"));
  });

  it("filters favorites, tags and categories on the loaded items", async () => {
    const fav = { ...summary, id: "f", title: "Fav", favorite: true, tags: [], kind: "note" as const };
    const client = fakeClient({
      items: vi.fn().mockResolvedValue([summary, fav]),
      tags: vi.fn().mockResolvedValue([{ name: "dev", count: 1 }]),
      categories: vi.fn().mockResolvedValue(categories),
    });
    const { result } = renderHook(() => useVaultData(client));
    await waitFor(() => expect(result.current.items).toHaveLength(2));
    expect(result.current.tags).toEqual([{ name: "dev", count: 1 }]);
    await waitFor(() => expect(result.current.categories.map((c) => c.kind)).toEqual(["login", "note"]));

    act(() => result.current.setScope({ type: "favorites" }));
    expect(result.current.items.map((i) => i.id)).toEqual(["f"]);
    expect(result.current.vaultId).toBeNull();
    act(() => result.current.setScope({ type: "tag", tag: "dev" }));
    expect(result.current.items.map((i) => i.id)).toEqual(["i1"]);
    act(() => result.current.setScope({ type: "all" }));
    act(() => result.current.setCategory("note"));
    expect(result.current.items.map((i) => i.id)).toEqual(["f"]);
    expect(result.current.categories.map((c) => c.kind)).toEqual(["login", "note"]);
  });

  it("lists the Archive from its own command", async () => {
    const archived = { ...summary, id: "z", archived: true };
    const client = fakeClient({ archivedItems: vi.fn().mockResolvedValue([archived]) });
    const { result } = renderHook(() => useVaultData(client));
    act(() => result.current.setScope({ type: "archive" }));
    await waitFor(() => expect(result.current.items).toEqual([archived]));
    expect(client.archivedItems).toHaveBeenLastCalledWith("");
  });

  it("exposes errors", async () => {
    const client = fakeClient({ vaults: vi.fn().mockRejectedValue({ code: "locked", message: "locked" }) });
    const { result } = renderHook(() => useVaultData(client));
    await waitFor(() => expect(result.current.error).toBeTruthy());
  });
});
