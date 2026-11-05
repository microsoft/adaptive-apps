FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build
WORKDIR /src
COPY TradingAgent.csproj ./
COPY Memory/AdvisorMemory.csproj Memory/
RUN dotnet restore TradingAgent.csproj
COPY . .
RUN dotnet publish TradingAgent.csproj -c Release --no-restore -o /app

FROM mcr.microsoft.com/dotnet/aspnet:8.0
LABEL com.azure.containerizationassist.createdby="containerization-assist"
RUN apt-get update && apt-get install -y --no-install-recommends curl && rm -r /var/lib/apt/lists
WORKDIR /app
COPY --from=build /app .
ENV ASPNETCORE_URLS=http://+:7000
EXPOSE 7000
USER app
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -fsS http://127.0.0.1:7000/health || exit 1
ENTRYPOINT ["dotnet", "TradingAgent.dll"]
