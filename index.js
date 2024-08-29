let running = true;

async function printForever() {
    while (running) {
        console.log(new Date().toISOString());

        await new Promise((resolve) => setTimeout(resolve, 1000));
    }
}

process.on("SIGINT", function () {
    console.log("Caught interrupt signal");

    running = false;
});

void (await printForever());
