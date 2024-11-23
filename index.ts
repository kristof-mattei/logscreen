// This app doesn't correctly refresh

class App {
    private running = true;

    public constructor() {
        process.on("SIGINT", this.stop.bind(this));
    }

    public stop(): void {
        console.log("Caught interrupt signal");

        this.running = false;
    }

    public async printForever(): Promise<void> {
        while (this.running) {
            console.log(new Date().toISOString());

            await new Promise((resolve) => setTimeout(resolve, 1000));
        }
    }
}

function main(): Promise<void> {
    const app = new App();

    return app.printForever();
}

await main();

export {};
