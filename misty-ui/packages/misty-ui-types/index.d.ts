declare global {
    type TimerHandle = number

    var console: {
        log: (...msg: any[]) => void       
    }

    function setTimeout(f: () => void, ms?: number): TimerHandle;
}

export {}