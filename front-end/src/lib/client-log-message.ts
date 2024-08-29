import type { UUID } from "node:crypto";

export interface ClientLogMessage {
    key: UUID;
    message: string;
    timestamp: Date;
}
