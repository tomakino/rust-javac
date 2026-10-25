package samples;

import java.io.BufferedReader;
import java.io.FileReader;
import java.io.IOException;
import java.io.Reader;
import java.util.Arrays;
import java.util.Collection;
import java.util.Comparator;
import java.util.function.Supplier;

public class Sample2 {
    private static final int[] PRIMES = {2, 3, 5, 7, 11};
    private static int counter;

    static {
        counter = PRIMES.length;
    }

    @Deprecated(since = "17")
    public enum Color implements Supplier<String> {
        RED {
            @Override
            public String get() {
                return "red";
            }
        },
        GREEN {
            @Override
            public String get() {
                return "green";
            }
        };

        private Color() {
        }
    }

    public final <T extends Comparable<? super T>> T maxOf(Collection<? extends T> values, T fallback) {
        return values.stream().max(Comparator.naturalOrder()).orElse(fallback);
    }

    public void io(String path) throws IOException {
        try (Reader reader = new FileReader(path);
             BufferedReader buffered = new BufferedReader(reader)) {
            String line;
            while ((line = buffered.readLine()) != null) {
                synchronized (this) {
                    counter += line.length();
                }
            }
        } finally {
            counter++;
        }
    }

    public int classify(int value) {
        int result = value > 0 ? (value & 1) == 0 ? 2 : 1 : value < 0 ? -1 : 0;
        outer:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                if (i * j > 2) {
                    break outer;
                }
                result += i - j;
            }
        }
        return result;
    }

    public static void main(String[] args) throws Exception {
        Sample2 sample = new Sample2();
        sample.io("build.gradle");
        System.out.println(sample.maxOf(Arrays.asList(3, 1, 2), 0));
        System.out.println(sample.classify(4));
        Runnable runnable = new Runnable() {
            @Override
            public void run() {
                System.out.println("anonymous");
            }
        };
        runnable.run();
        Supplier<Object> constructor = Sample2::new;
        Object instance = constructor.get();
        System.out.println(instance != null && Color.RED.get() != null);
    }
}
