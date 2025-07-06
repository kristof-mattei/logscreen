import React from "react";
import type { ErrorInfo, PropsWithChildren, ReactNode } from "react";

// eslint-disable-next-line @typescript-eslint/no-empty-object-type -- Consistency
interface ErrorBoundaryProperties {}

interface ErrorBoundaryState {
    hasError: boolean;
    error?: unknown;
}

export class ErrorBoundary extends React.Component<PropsWithChildren<ErrorBoundaryProperties>, ErrorBoundaryState> {
    public constructor(properties: ErrorBoundaryProperties) {
        super(properties);
        this.state = { hasError: false };
    }

    public static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
        return { hasError: true, error };
    }

    public override componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
        console.error("Error caught by ErrorBoundary:", error, errorInfo);
        console.log("props:", this.props);
    }

    public override render(): ReactNode {
        if (this.state.hasError) {
            return (
                <div className="p-4 bg-red-200 text-red-800">
                    <p>Something is not right in here.</p>
                    <p>{String(this.state.error)}.</p>
                </div>
            );
        }

        return this.props.children ?? <></>;
    }
}
