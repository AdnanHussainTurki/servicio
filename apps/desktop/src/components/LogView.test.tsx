import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useStore } from "../store";
import { LogView } from "./LogView";

describe("LogView", () => {
  beforeEach(() => useStore.getState().reset());
  it("renders buffered log lines for the worker", () => {
    useStore.getState().applyEvent({ kind: "log", worker: "q", instance: 0, stream: "stdout", line: "hello world" });
    render(<LogView worker="q" />);
    expect(screen.getByText(/hello world/)).toBeDefined();
  });
});
