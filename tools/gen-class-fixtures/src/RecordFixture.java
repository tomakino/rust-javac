public record RecordFixture(int x, String name) {
    public RecordFixture {
        if (x < 0) throw new IllegalArgumentException();
    }
    public int doubled() {
        return x * 2;
    }
}
