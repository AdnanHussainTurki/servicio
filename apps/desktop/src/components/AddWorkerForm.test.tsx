import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AddWorkerForm } from "./AddWorkerForm";

describe("AddWorkerForm", () => {
  it("submits a well-formed add_worker spec", () => {
    const onSubmit = vi.fn();
    render(<AddWorkerForm onSubmit={onSubmit} onCancel={() => {}} />);
    fireEvent.change(screen.getByLabelText(/name/i), { target: { value: "q" } });
    fireEvent.change(screen.getByLabelText(/command/i), { target: { value: "php" } });
    fireEvent.change(screen.getByLabelText(/args/i), { target: { value: "artisan queue:work" } });
    fireEvent.change(screen.getByLabelText(/working dir/i), { target: { value: "/srv/app" } });
    fireEvent.click(screen.getByText(/create/i));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    const spec = onSubmit.mock.calls[0][0];
    expect(spec.name).toBe("q");
    expect(spec.command).toBe("php");
    expect(spec.args).toEqual(["artisan", "queue:work"]);
    expect(spec.run_mode).toEqual({ type: "daemon", concurrency: 1 });
    expect(spec.enabled).toBe(true);
  });
});
