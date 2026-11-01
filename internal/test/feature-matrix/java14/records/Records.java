public class Records {
    record Point(int x, int y) {
        Point {
            if (x < 0) {
                throw new IllegalArgumentException("negative x");
            }
        }

        int sum() {
            return x + y;
        }
    }

    public static void main(String[] args) {
        Point point = new Point(1, 2);
        System.out.println(point + " " + point.sum() + " " + point.equals(new Point(1, 2)));
    }
}
