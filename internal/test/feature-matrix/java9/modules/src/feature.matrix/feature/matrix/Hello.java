package feature.matrix;

public class Hello implements Runnable {
    public Hello() {
    }

    @Override
    public void run() {
        System.out.println(Helper.message());
    }
}
